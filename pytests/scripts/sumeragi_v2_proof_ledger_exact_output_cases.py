# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


_TERMINAL_LANE_MUTATIONS = (
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "self.output_guard.close_admission_for_restart();", "",
     "close malformed ownership"),
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "BlockMessage::LaneBlockProposal(proposal) => Some(proposal),",
     "BlockMessage::LaneBlockProposal(proposal) => None,",
     "revalidate finalized proposal"),
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "self.finalized_autonomous_ingress_payload_for_proposal_or_fail_stop(proposal)\n                .is_err()",
     "self.finalized_autonomous_ingress_payload_for_proposal_or_fail_stop(proposal)\n                .is_ok()",
     "revalidate finalized proposal"),
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "if self.decision_pending() {\n            let finalized_body",
     "if false {\n            let finalized_body",
     "Decision bodies before dispatch"),
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "self.finalized_autonomous_ingress_payload_or_fail_stop(body)\n                    .is_err()",
     "self.finalized_autonomous_ingress_payload_or_fail_stop(body)\n                    .is_ok()",
     "Decision bodies before dispatch"),
    ("V2LaneWorkAdapter::accept_lane_message_owned",
     "if self.output_guard.restart_required() {", "if false {",
     "preserving restart admission closure"),
    ("From<&LaneBlockProposalV1> for AutonomousLanePayloadKey::from",
     "dataspace_id: descriptor.dataspace_id,", "dataspace_id: DataSpaceId::UNIVERSAL,",
     "full proposal route, incarnation, and lane height"),
    ("From<&LaneBlockProposalV1> for AutonomousLanePayloadKey::from",
     "lane_incarnation: descriptor.lane_incarnation,", "lane_incarnation: Hash::prehashed([0; 32]),",
     "full proposal route, incarnation, and lane height"),
    ("From<&LaneBlockProposalV1> for AutonomousLanePayloadKey::from",
     "lane_block_height: descriptor.lane_block_height,", "lane_block_height: descriptor.proposal_height,",
     "full proposal route, incarnation, and lane height"),
    ("validate_terminal_autonomous_availability", "if *actual != expected {",
     "if false && *actual != expected {", "bind Prepare READY"),
    ("validate_terminal_autonomous_availability", "(CertPhase::Commit, None) => Ok(()),",
     "(CertPhase::Prepare | CertPhase::Commit, None) => Ok(()),", "require Commit without READY"),
    ("validate_terminal_autonomous_vote",
     "crate::lane_consensus::validate_vote_matches_proposal(vote, &payload.origin_proposal)\n        .map_err(|error| error.to_string())?;",
     "let _ = &payload.origin_proposal;", "READY committee PoPs, outer signature"),
    ("validate_terminal_autonomous_vote",
     "vote.validate_ingress(vote.body.phase)\n        .map_err(|error| error.to_string())?;",
     "let _ = vote.body.phase;", "READY committee PoPs, outer signature"),
    ("validate_terminal_autonomous_qc",
     "validate_winning_lane_qc(qc, &payload.origin_proposal, signer_pops)?;",
     "let _ = signer_pops;", "complete aggregate before exact availability"),
    ("validate_terminal_autonomous_qc", "qc.body.phase,", "CertPhase::Commit,",
     "complete aggregate before exact availability"),
    ("V2LaneWorkAdapter::insert_lane_vote",
     ".certified_autonomous_lane_block_is_globally_applied(",
     ".certified_autonomous_lane_block_predecessor_is_globally_applied(",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_vote",
     "validate_terminal_autonomous_vote(&vote, payload).is_ok()", "true",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_vote", "return if validate_terminal_autonomous_vote",
     "let _outcome = if validate_terminal_autonomous_vote",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_qc",
     ".certified_autonomous_lane_block_is_globally_applied(",
     ".certified_autonomous_lane_block_predecessor_is_globally_applied(",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_qc",
     "validate_terminal_autonomous_qc(&qc, payload, &pops).is_ok()", "true",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_qc", "return if validate_terminal_autonomous_qc",
     "let _outcome = if validate_terminal_autonomous_qc",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_qc", "move_clone_before_terminal", "",
     "authenticate exact applied replay before the first cache clone"),
    ("V2LaneWorkAdapter::insert_lane_certificate", ".any(|(qc, phase)| {",
     ".all(|(qc, phase)| {", "availability before any historical shortcut"),
    ("V2LaneWorkAdapter::insert_lane_certificate", "move_availability_after_historical", "",
     "availability before any historical shortcut"),
    ("V2LaneWorkAdapter::canonical_finalized_autonomous_payload_for_vote_body",
     ".certified_autonomous_lane_block_is_globally_applied(proposal)",
     ".certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)",
     "either exact own application or the exact applied predecessor"),
    ("V2LaneWorkAdapter::canonical_finalized_autonomous_payload_for_vote_body",
     ".map_err(|error| error.to_string())?\n                && !self",
     ".map_err(|error| error.to_string())?\n                || !self",
     "either exact own application or the exact applied predecessor"),
    ("V2LaneWorkAdapter::canonical_finalized_autonomous_payload_for_vote_body",
     "let proposal = &payload.origin_proposal;",
     "let mut detached = payload.origin_proposal.clone();\n            detached.payload_block_hint = None;\n            let proposal = &detached;",
     "attach the exact global hint"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     "self.lane_sessions.retained_vote_bodies()", "Vec::new()", "bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     "self.lane_sessions.rollover_slots()", "BTreeSet::new()", "bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     ".read_certified_lane_block_artifact_read_only(lane_id, lane_block_height)",
     ".read_certified_lane_block_artifact(lane_id, lane_block_height)", "bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     "Kura::validate_certified_lane_block_artifact(&artifact)", "Ok::<(), &str>(())",
     "authenticate bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     ".canonical_finalized_autonomous_payload_for_vote_body(&body)\n                .map_err(V2LaneWorkError::Persistence)?",
     ".canonical_finalized_autonomous_payload_for_vote_body(&body).unwrap_or(None)",
     "authenticate bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     "for qc in [&artifact.prepare_qc, &artifact.commit_qc] {",
     "for qc in [] as [&LaneBlockQcV1; 0] {", "authenticate bounded read-only candidates"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     ".certified_autonomous_lane_block_is_globally_applied(&proposal)",
     ".certified_autonomous_lane_block_predecessor_is_globally_applied(&proposal)",
     "exact application"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     ".is_some_and(|existing| existing != &proposal)",
     ".is_some_and(|existing| !existing.same_consensus_identity(&proposal))", "full slots atomically"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     ".retire_applied_proposals(&proposals)", ".retire_applied_proposals(&[])", "full slots atomically"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions", "move_cleanup_before_cache", "",
     "then clean only selected volatile owners"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions",
     "lane_incarnation: key.lane_incarnation,", "lane_incarnation: Hash::prehashed([0; 32]),",
     "only selected volatile owners"),
    ("V2LaneWorkAdapter::retire_applied_autonomous_sessions", "Ok(retired)",
     "self.pending_committed_lanes.clear();\n        self.committed_lane_outputs.clear();\n        Ok(retired)",
     "without consuming outputs"),
    ("V2LaneWorkAdapter::drive_lane_sessions", "self.retire_applied_autonomous_sessions()",
     "Ok::<usize, V2LaneWorkError>(0)", "close admission on failure before its first signing work"),
    ("V2LaneWorkAdapter::persist_anchored_sessions", "self.retire_applied_autonomous_sessions()?;",
     "let _ = self.retire_applied_autonomous_sessions();", "inside its fail-stop operation before hydration"),
    ("V2LaneWorkAdapter::persist_anchored_sessions", "move_retirement_before_guard", "",
     "inside its fail-stop operation before hydration"),
    ("V2LaneWorkAdapter::persist_anchored_sessions",
     "self.retire_applied_autonomous_sessions()?;\n        self.hydrate_canonical_lane_artifacts()?;",
     "self.hydrate_canonical_lane_artifacts()?;\n        self.retire_applied_autonomous_sessions()?;",
     "inside its fail-stop operation before hydration"),
    ("V2LaneWorkAdapter::proposal_can_progress",
     ".certified_autonomous_lane_block_is_globally_applied(proposal)",
     ".certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)",
     "reject exact own application"),
    ("V2LaneWorkAdapter::proposal_can_progress",
     "if (historical\n            || self.autonomous_payload_is_expected_for(proposal)?\n            || finalized_autonomous)",
     "if true", "only for authenticated autonomous roles"),
    ("V2LaneWorkAdapter::proposal_can_progress",
     "|| finalized_autonomous)", "|| finalized_observer)",
     "only for authenticated autonomous roles"),
    ("V2LaneWorkAdapter::proposal_can_progress",
     "historical\n            || self.autonomous_payload_is_expected_for(proposal)?\n            || finalized_autonomous",
     "finalized_autonomous", "only for authenticated autonomous roles"),
    ("LaneBlockSessionCache::preflight_canonical_evidence",
     "for ((slot, signer), locked_proposal_hash) in &self.commit_vote_locks {",
     "for ((slot, signer), locked_proposal_hash) in &BTreeMap::new() {", "orphan Commit quorums"),
    ("LaneBlockSessionCache::preflight_canonical_evidence", "signers.len() >= quorum",
     "signers.len() > quorum", "orphan Commit quorums"),
    ("LaneBlockSessionCache::preflight_canonical_evidence",
     "descriptor.validator_set.binary_search(signer).is_err()",
     "descriptor.validator_set.binary_search(signer).is_ok()", "exact-committee orphan Commit quorums"),
    ("LaneBlockSessionCache::preflight_canonical_evidence", "if session_has_quorum_certificate(session)",
     "if session.commit_qc.is_some()", "proposal-less Prepare or Commit QCs"),
    ("LaneBlockSessionCache::retire_applied_proposals",
     "validate_lane_block_proposal(proposal)\n                .map_err(LaneBlockSessionError::InvalidProposal)?;",
     "let _ = proposal;", "complete exact full-slot target set before mutation"),
    ("LaneBlockSessionCache::retire_applied_proposals", ".is_some_and(|existing| existing != proposal)",
     ".is_some_and(|existing| !existing.same_consensus_identity(proposal))", "complete exact full-slot target set"),
    ("LaneBlockSessionCache::retire_applied_proposals", "move_preflight_after_retirement", "",
     "target set before mutation"),
    ("LaneBlockSessionCache::retire_applied_proposals",
     "lane_incarnation: descriptor.lane_incarnation,", "lane_incarnation: Hash::prehashed([0; 32]),",
     "exact full-slot target set"),
    ("LaneBlockSessionCache::retire_applied_proposals",
     "self.commit_vote_locks\n            .retain(|(slot, _), _| !canonical.contains_key(slot));",
     "self.commit_vote_locks.clear();", "preserve unselected sessions, locks"),
    ("LaneBlockSessionCache::retire_applied_proposals", ".or_insert(*key);",
     ".and_modify(|owner| *owner = *key).or_insert(*key);", "claims, recency, and capacity"),
    ("LaneBlockSessionCache::retire_applied_proposals", "Ok(before.saturating_sub(after))",
     "self.capacity += 1;\n        Ok(before.saturating_sub(after))", "recency, and capacity"),
    ("LaneBlockSessionCache::retire_applied_proposals",
     "self.order.retain(|key| retained_sessions.contains_key(key));", "self.order.clear();",
     "recency, and capacity"),
    ("LaneBlockSessionCache::retained_vote_bodies",
     ".or_else(|| session.prepare_qc.as_ref().map(|qc| qc.body.clone()))", "",
     "every retained proposal or vote/QC body"),
    ("LaneBlockSessionCache::retained_vote_bodies",
     ".or_else(|| session.commit_qc.as_ref().map(|qc| qc.body.clone()))", "",
     "every retained proposal or vote/QC body"),
    ("LaneBlockSessionCache::retained_vote_bodies", ".commit_votes", ".prepare_votes",
     "every retained proposal or vote/QC body"),
    ("LaneBlockSessionCache::retained_vote_bodies",
     "session.commit_qc.as_ref().map(|qc| qc.body.clone())",
     "session.commit_qc.as_ref().map(|qc| { let mut body = qc.body.clone(); body.proposal_height += 1; body })",
     "without mutation or inferred global heights"),
    ("LaneBlockSessionCache::retain_canonical_rollover_evidence", "remove_shared_preflight", "",
     "share the complete canonical quorum preflight"),
    ("validate_vote_matches_proposal",
     "|| availability_vote\n                    .validate_against_validator_set(&proposal.descriptor.validator_set)\n                    .is_err()",
     "|| false", "exact complete committee and its PoPs"),
    ("State::certified_autonomous_lane_block_is_globally_applied", "replace_exact_frontier_with_height", "",
     "exact route/incarnation frontier identity"),
    ("State::certified_autonomous_lane_block_is_globally_applied",
     "receipt.proposal == *proposal",
     "true", "exact authenticated merge receipt"),
    ("State::certified_autonomous_lane_block_is_globally_applied",
     "descriptor.lane_incarnation,\n        )?;",
     "descriptor.lane_incarnation,\n        ).unwrap_or((0, None));",
     "fail closed on malformed frontier bytes"),
)


def _terminal_lane_order_mutation(item, mutation: str) -> tuple[str, str]:
    """Move real complete source blocks to exercise semantic boundary ordering."""

    old = item.source
    if mutation == "move_clone_before_terminal":
        clone = "        let mut next_sessions = self.lane_sessions.clone();\n"
        assert old.count(clone) == 1
        new = old.replace(clone, "", 1).replace(
            "        if let Some(payload) = finalized_payload.as_ref()",
            clone + "        if let Some(payload) = finalized_payload.as_ref()", 1,
        )
    elif mutation == "move_availability_after_historical":
        start = old.index("        if let Some(payload) = finalized_payload.as_ref()")
        end = old.index("        if proposal.descriptor.proposal_height < self.context.height {", start)
        block = old[start:end]
        new = old.replace(block, "", 1).replace(
            "        if !self\n            .proposal_body_available(&proposal)",
            block + "        if !self\n            .proposal_body_available(&proposal)", 1,
        )
    elif mutation == "move_cleanup_before_cache":
        start = old.index("        let retired = self")
        end = old.index("        self.lane_ready_authorizations.retain", start)
        block = old[start:end]
        new = old.replace(block, "", 1).replace("        Ok(retired)", block + "        Ok(retired)", 1)
    elif mutation == "move_retirement_before_guard":
        retire = "        self.retire_applied_autonomous_sessions()?;\n"
        assert old.count(retire) == 1
        new = old.replace(retire, "", 1).replace(
            "        let output_guard = Arc::clone(&self.output_guard);",
            retire + "        let output_guard = Arc::clone(&self.output_guard);", 1,
        )
    elif mutation == "move_preflight_after_retirement":
        preflight = "        self.preflight_canonical_evidence(|slot| canonical.get(&slot).copied())?;\n"
        assert old.count(preflight) == 1
        new = old.replace(preflight, "", 1).replace(
            "        Ok(before.saturating_sub(after))",
            preflight + "        Ok(before.saturating_sub(after))", 1,
        )
    elif mutation == "remove_shared_preflight":
        start = old.index("        self.preflight_canonical_evidence(|slot| {")
        end = old.index("        let mut retained_sessions = BTreeMap::new();", start)
        new = old[:start] + old[end:]
    elif mutation == "replace_exact_frontier_with_height":
        start = old.index("        if frontier\n            == (")
        end = old.index("        {\n            return Ok(true);", start)
        new = old[:start] + "        if frontier.0 >= descriptor.lane_block_height\n" + old[end:]
    else:
        raise AssertionError(mutation)
    assert new != old, mutation
    return old, new


@pytest.mark.parametrize(("qualified", "old", "new", "expected_error"), _TERMINAL_LANE_MUTATIONS)
def test_terminal_lane_source_mutations_survive_digest_refresh(
    tmp_path: Path, qualified: str, old: str, new: str, expected_error: str,
) -> None:
    """Every copied production baseline passes before a fully rehashed semantic mutant fails."""

    module = load_checker()
    selected = [
        (relative, owner, name)
        for relative, declarations in module._TERMINAL_LANE_SOURCE_OWNERS.items()
        for owner, name in declarations
        if (f"{owner}::{name}" if owner else name) == qualified
    ]
    assert len(selected) == 1, qualified
    relative, owner, name = selected[0]
    path = tmp_path / relative
    path.parent.mkdir(parents=True)
    shutil.copyfile(ROOT_DIR / relative, path)
    context = (module.rust_code_tokens(f"impl {owner}"),) if owner else ()
    errors = []
    item = module._require_terminal_lane_owner(
        path, path.read_text(encoding="utf-8"), owner, name, errors,
    )
    assert item is not None
    baseline_digest = module._PRODUCTION_TERMINAL_LANE_ITEM_SHA256.get(
        qualified, module._rust_item_token_sha256(item),
    )
    if qualified == "V2LaneWorkAdapter::persist_anchored_sessions":
        baseline_digest = module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256[qualified]
    if qualified == "V2LaneWorkAdapter::accept_lane_message_owned":
        baseline_digest = module._PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256[
            "lane::accept_lane_message_owned"
        ]
    module._require_rust_item_token_sha256(path, item, baseline_digest, qualified, errors)
    module._require_terminal_lane_source_contracts(path, {qualified: item}, errors)
    assert not errors, errors
    if old.startswith("move_") or old in ("remove_shared_preflight", "replace_exact_frontier_with_height"):
        old, new = _terminal_lane_order_mutation(item, old)
    mutate_rust_item_source_in_context(module, path, name, context, old, new)
    # Existing owners are not silently resealed in production. This local table
    # verifies each mutant's complete item digest independently of its semantics.
    refreshed = {qualified: baseline_digest}
    bindings = [(refreshed, qualified)]
    if qualified in module._PRODUCTION_TERMINAL_LANE_ITEM_SHA256:
        bindings.append((module._PRODUCTION_TERMINAL_LANE_ITEM_SHA256, qualified))
    if qualified == "V2LaneWorkAdapter::persist_anchored_sessions":
        bindings.append((module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256, qualified))
    if qualified == "V2LaneWorkAdapter::accept_lane_message_owned":
        bindings.append((
            module._PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256,
            "lane::accept_lane_message_owned",
        ))
    original = rebind_reviewed_rust_item_digests(module, path, name, context, tuple(bindings))
    try:
        errors = []
        mutated = next(
            candidate for candidate in module.rust_items(path.read_text(encoding="utf-8"), name)
            if candidate.brace_context == context
        )
        assert refreshed[qualified] != baseline_digest
        module._require_rust_item_token_sha256(path, mutated, refreshed[qualified], qualified, errors)
        module._require_terminal_lane_source_contracts(path, {qualified: mutated}, errors)
    finally:
        restore_reviewed_rust_item_digests(original)
    assert any(expected_error in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


def test_terminal_lane_production_source_is_bound() -> None:
    """The independent loader binds real adapter, cache, and State production owners."""

    module = load_checker()
    errors = module._terminal_lane_source_fidelity_errors(ROOT_DIR)
    assert not errors, errors


def _worker_ownership_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Rehash real worker constructors, exact claims and opaque proof owners."""

    module = load_checker()
    mutations = (
        ("proof_layout", "    network_id: NetworkId,", "    pub(crate) network_id: NetworkId,", "no mutable or forgeable source identity"),
        ("proof_layout", "    exact_output_hash: HashOf<NetworkMessage>,", "    pub(crate) exact_output_hash: HashOf<NetworkMessage>,", "no mutable or forgeable source identity"),
        ("proof_network_id", "self.network_id", "NetworkId::default()", "immutable minted network"),
        ("proof_source_round", "self.source_round", "wire::ConsensusRound { height: 0, ..self.source_round }", "immutable minted height and context"),
        ("proof_covers_message", "if &self.network_id != expected_network_id {", "if false {", "exact network, response family"),
        ("proof_covers_message", "response.request_hash == self.request_hash", "true", "request, source, responder"),
        ("proof_covers_message", "&& response.manifest.round == self.source_round", "&& true", "request, source, responder"),
        ("proof_covers_message", "&& response.manifest.subject == self.source_subject", "&& true", "request, source, responder"),
        ("proof_covers_message", "&& response.responder == self.responder", "&& true", "request, source, responder"),
        ("proof_covers_message", "&& message.cached_exact_output_hash() == Some(self.exact_output_hash)", "&& true", "warmed whole-message hash"),
        ("network_exact_output_hash", "HashOf::new(self))", "HashOf::new(envelope.as_message()))", "canonical whole-message cached identity"),
        ("durable_history_source_covers", "let [message] = messages else {", "let Some(message) = messages.first() else {", "non-singleton, exact-only or foreign"),
        ("durable_history_source_covers", "if message.progress_reconstruction() != ProgressReconstruction::Retransmit {", "if false {", "non-singleton, exact-only or foreign"),
        ("durable_history_source_covers", "proof.source_round().height > maximum_source_height", "proof.source_round().height < maximum_source_height", "canonical Kura block through the prepared proof"),
        ("durable_history_source_covers", "if network_id != source_network_id", "if false", "canonical Kura block through the prepared proof"),
        ("durable_history_source_covers", "|| proof.network_id() != *source_network_id", "|| false", "canonical Kura block through the prepared proof"),
        ("durable_history_source_covers", "|| !proof.covers_message_in_network(source_network_id, message)", "|| false", "canonical Kura block through the prepared proof"),
        ("durable_history_source_covers", "proof.covers_message_in_network(source_network_id, message)", "proof.covers_message_in_network(network_id, message)", "canonical Kura block through the prepared proof"),
        ("validate_fanout", "let [message] = messages else {", "let Some(message) = messages.first() else {", "one exact message, target, network"),
        ("validate_fanout", "if peers != std::slice::from_ref(target)\n                    || proof.network_id()", "if false\n                    || proof.network_id()", "one exact message, target, network"),
        ("validate_fanout", "|| proof.network_id() != *network_id", "|| false", "one exact message, target, network"),
        ("validate_fanout", "|| !proof.covers_message_in_network(network_id, message)", "|| false", "one exact message, target, network"),
        ("post_durable_history_response_with_routes", "return Err(\n                    \"historical body output must cross the bounded prepared-worker seam\".to_owned(),\n                );", "return Ok(());", "rejects body responses before encoding"),
        ("post_durable_history_response_with_routes", "&& response.responder == self.local_peer", "&& true", "local responder, exact hash"),
        ("post_durable_history_response_with_routes", "response.certificate.round.height <= self.context.height", "true", "non-future creation scope"),
        ("post_durable_history_response_with_routes", "rollover_claim.validate_fanout(&messages, &peers)?;", "", "Kura before transferring output ownership"),
        ("post_durable_history_response_with_routes", "durable_history_source_covers(", "durable_history_source_covers_unchecked(", "Kura before transferring output ownership"),
        ("applied_height_reconstruction_covers", "rollover_claim.validate_fanout(messages, peers)?;", "", "before any role shortcut"),
        ("applied_height_reconstruction_covers", "if !scope.covers(artifact) {", "if false {", "exact creation scope before any role shortcut"),
        ("applied_height_reconstruction_covers", "            &artifact.height_context.network_id,", "            &wire::NetworkId::default(),", "artifact network and height into prepared-proof"),
        ("applied_height_reconstruction_covers", "            artifact.height,", "            u64::MAX,", "artifact network and height into prepared-proof"),
        ("applied_height_reconstruction_covers", "round.context_id == context_id && round.height == height", "round.context_id == context_id", "both context and height"),
        ("applied_height_reconstruction_covers", "wire::ConsensusMessageV2Payload::PayloadChunk(_) => false,", "wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => round_matches(manifest.round),\n                    wire::ConsensusMessageV2Payload::PayloadChunk(_) => false,", "standalone manifest cannot reenter"),
        ("applied_height_reconstruction_covers", "wire::ConsensusMessageV2Payload::PayloadChunk(_) => false,", "wire::ConsensusMessageV2Payload::PayloadChunk(_) => true,", "excluding unclaimed chunks"),
        ("validate_fanout_bounds", "fanout.message_hashes.len() != fanout.messages.len()", "false", "complete immutable indexes"),
        ("validate_fanout_bounds", "message.exact_output_hash() != *expected_hash", "message.exact_output_hash() == *expected_hash", "canonical cached hashes and traffic classes"),
        ("validate_fanout_bounds", "|| exact_output_class(message).as_ref() != Ok(expected_class)", "|| false", "canonical cached hashes and traffic classes"),
        ("validate_fanout_bounds", "let _ = fanout.outstanding_sources()?;", "", "every future source and FIFO before capacity"),
        ("start", "            None,\n            key_pair,", "            kagemusha_mint_finality_authority,\n            key_pair,", "without fabricating lifecycle authority"),
        ("start", "            exact_output_handoff_owner,", "            DurableExactOutputServiceOwner::default(),", "exact authorities and capacities"),
        ("start_inner", ".begin_fail_stop_operation()", ".begin_operation()", "holds fail-stop authority"),
        ("start_inner", "consensus_io_capacity == 0 || auxiliary_io_capacity == 0 || orphan_chunk_capacity == 0", "false", "validates capacity and tag before side effects"),
        ("start_inner", "if initial_tag.height() != context.height {", "if false {", "validates capacity and tag before side effects"),
        ("start_inner", ".map(|entry| entry.validator.clone())", ".filter(|_| false).map(|entry| entry.validator.clone())", "complete frozen roster"),
        ("start_inner", "            kagemusha_mint_finality_authority,", "            None,", "explicit finality authority under its guard"),
        ("start_inner", "            Arc::clone(&output_guard),", "            Arc::new(ConsensusOutputGuard::new()),", "explicit finality authority under its guard"),
        ("start_inner", "let lifecycle_body_store_identity = body_store.instance_identity();", "let lifecycle_body_store_identity = body_store.instance_identity();\n        std::fs::create_dir_all(\"discarded-chunks\")?;", "unused raw chunk-directory ownership"),
        ("start_inner", "        construction.complete();\n        service.clean_teardown = false;", "        service.clean_teardown = false;\n        construction.complete();", "only after the full service exists"),
    )
    # Preserve every required clause while moving the actual geometry preflight
    # after worker creation, so a token-presence-only checker cannot pass.
    relative, name, context = module._WORKER_OWNERSHIP_RECONCILIATION_OWNERS["start_inner"]
    constructor = next(item for item in module.rust_items(
        (ROOT_DIR / "crates/iroha_core/src/sumeragi" / relative).read_text(), name)
        if item.brace_context == (module.rust_code_tokens(context),)).source
    geometry_start = constructor.index("        let shared_pending_ownership_unit_capacity =")
    geometry_end = constructor.index("        let durable_history =")
    spawn_end = constructor.index("        let mut service =")
    mutations += (("start_inner", constructor[geometry_start:spawn_end],
        constructor[geometry_end:spawn_end] + constructor[geometry_start:geometry_end],
        "validates ownership geometry before spawning"),)

    def copy_source(relative: str, copy_root: Path, copied: set[str]) -> None:
        if relative in copied:
            return
        copied.add(relative)
        path = copy_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, path)
        for child in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative, ()):
            copy_source((Path(relative).parent / child).as_posix(), copy_root, copied)

    for index, (key, old, new, expected) in enumerate(mutations):
        copy_root = tmp_path / f"worker-ownership-{index:02d}"
        copied: set[str] = set()
        for relative, _name, _context in module._WORKER_OWNERSHIP_RECONCILIATION_OWNERS.values():
            copy_source(f"crates/iroha_core/src/sumeragi/{relative}", copy_root, copied)
        baseline = module._worker_ownership_reconciled_source_fidelity_errors(copy_root)
        assert not baseline, (key, old, baseline)
        relative, name, context = module._WORKER_OWNERSHIP_RECONCILIATION_OWNERS[key]
        path = copy_root / "crates/iroha_core/src/sumeragi" / relative
        brace_context = (module.rust_code_tokens(context),) if context else ()
        parser = module.rust_struct_items if key == "proof_layout" else module.rust_items
        source = path.read_text()
        item = next(item for item in parser(source, name) if item.brace_context == brace_context)
        assert item.source.count(old) == 1, (key, old, item.source.count(old))
        path.write_text(source.replace(item.source, item.source.replace(old, new, 1), 1))
        item = next(item for item in parser(path.read_text(), name) if item.brace_context == brace_context)
        digest = module._rust_item_token_sha256(item)
        seals = module._WORKER_OWNERSHIP_RECONCILIATION_EXISTING_SEALS.get(key,
            (("_PRODUCTION_WORKER_OWNERSHIP_RECONCILIATION_ITEM_SHA256", key),))
        restored = []
        try:
            for mapping_name, seal_key in seals:
                mapping = getattr(module, mapping_name)
                assert digest != mapping[seal_key], (key, old)
                restored.append((mapping, seal_key, mapping[seal_key]))
                mapping[seal_key] = digest
            errors = module._worker_ownership_reconciled_source_fidelity_errors(copy_root)
            assert any(expected in error for error in errors), (key, old, errors)
            assert not any("exact reviewed token digest" in error for error in errors), (key, old, errors)
        finally:
            for mapping, seal_key, original in restored:
                mapping[seal_key] = original


def _worker_ack_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Keep exact payload, post-lock advert and receipt retirement checks after rehash."""

    module = load_checker()
    mutations = (
        ("classified_with_route_history", ".map(NetworkMessage::exact_output_hash)", ".map(HashOf::new)", "canonical cached hash"),
        ("classified_with_route_history", "            reply_routes,", "            reply_routes: None,", "full route geometry"),
        ("classified_with_route_history", "fanout.rebuild_current_source_targets()?;", "", "rebuilds exact source ownership"),
        ("plan_fanout_removal", "message.exact_output_hash() != *expected", "message.exact_output_hash() == *expected", "authenticates every cached payload"),
        ("plan_fanout_removal", "fanout.message_hashes.len() != fanout.messages.len()", "false", "authenticates every cached payload"),
        ("plan_fanout_removal", "validate_removed(fanout)?;", "", "validates each selected FIFO owner"),
        ("plan_fanout_removal", "|| current_reservations != self.reservation_owner_counts", "|| false", "inconsistent source and reservation indexes"),
        ("handoff_applied_height_to_durable_reconstruction", "message.exact_output_hash() != *expected_hash", "message.exact_output_hash() == *expected_hash", "preflight every pinned payload"),
        ("handoff_applied_height_to_durable_reconstruction", "current.data.exact_output_hash() != *expected_hash", "current.data.exact_output_hash() == *expected_hash", "each returned actor post"),
        ("handoff_applied_height_to_durable_reconstruction", "                durable_lane_authority,", "                None,", "durable reconstruction authority"),
        ("handoff_applied_height_to_durable_reconstruction", "|| self.shared_ownership_units != expected_shared_ownership_units", "|| false", "before atomically clearing the corridor"),
        ("handoff_applied_height_to_durable_reconstruction", "let sidecar_completions = self.admitted_sidecar_chunks.len();", "self.fanouts.clear();\n        let sidecar_completions = self.admitted_sidecar_chunks.len();", "before atomically clearing the corridor"),
        ("handoff_applied_height_to_durable_reconstruction", ".checked_add(sidecar_completions)", ".checked_add(0)", "checked sidecar count"),
        ("retain_returned", "if target.parked {", "if false {", "rejects parked or flushing sources"),
        ("retain_returned", "if target.pending_flush.is_some() {", "if false {", "rejects parked or flushing sources"),
        ("retain_returned", ".get(target.message_index)", ".get(0)", "exact payload index"),
        ("retain_returned", "post.data.exact_output_hash() != *expected_hash", "post.data.exact_output_hash() == *expected_hash", "exact pinned payload identity"),
        ("retain_returned", "target.ticket = ticket;", "target.ticket = None;", "exact pinned payload identity"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "if reply_routes.semantic_target() != &peer {", "if false {", "exact semantic target, source routes and ingress claim"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "            ingress_ownership,", "            None,", "exact semantic target, source routes and ingress claim"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "let ownership = {\n            let mut pending = self.lock_pending_exact_output()?;", "let mut pending = self.lock_pending_exact_output()?;\n        let ownership = {", "releases the corridor lock"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "if self.exact_output_handoff_owner.is_sealed() {", "if false {", "schedules every released advert"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "if ownership == ExactFanoutOwnership::Owned {", "if ownership == ExactFanoutOwnership::SourceRetained {", "schedules every released advert"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "                .map(|_| ownership)", "                ?;\n                Ok(ownership)", "even a failed drive result"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "self.schedule_released_kura_replica_advert_heights(released_kura_replica_advert_heights)?;", "self.schedule_released_kura_replica_advert_heights(BTreeSet::new())?;", "schedules every released advert"),
        ("enqueue_owned_exact_reply_routes_while_guarded", "self.schedule_released_kura_replica_advert_heights(released_kura_replica_advert_heights)?;", "let _ = self.schedule_released_kura_replica_advert_heights(released_kura_replica_advert_heights);", "even a failed drive result"),
        ("enqueue_exact_fanout_while_guarded_collecting_released_adverts", "PendingExactFanout::claimed(messages, peers, rollover_claim)", "PendingExactFanout::classified(messages, peers)", "typed claim"),
        ("enqueue_exact_fanout_while_guarded_collecting_released_adverts", "if self.exact_output_handoff_owner.is_sealed() {", "if false {", "typed claim"),
        ("enqueue_exact_fanout_while_guarded_collecting_released_adverts", "self.drive_pending_exact_output(&mut pending, released_kura_replica_advert_heights)", "self.drive_pending_exact_output(&mut pending, &mut BTreeSet::new())", "preserving released adverts"),
        ("finish_height", "Ok(_) if !self.exact_output_handoff_owner.is_sealed()", "Ok(_) if false", "sealed empty corridor"),
        ("finish_height", "Ok(pending) if pending.is_pending()", "Ok(pending) if false", "sealed empty corridor"),
        ("finish_height", "self.output_guard.activate_restart_required();", "", "activates restart before retirement"),
        ("finish_height", "                receipt,\n                cleanup: supervisor.submission(),", "                receipt,\n                cleanup: supervisor.submission(),\n                chunk_root: self.chunk_root.clone(),", "without a raw directory path"),
        ("finish_height", "drop(retirement_enqueue_permit);", "", "drops its permit before waiting"),
        ("finish_height", "command = returned;", "command = V2IoCommand::Shutdown;", "preserves a full returned retirement command"),
        ("finish_height", "recv_cleanup_completion(&io, deadline)", "recv_cleanup_completion(&io, Instant::now())", "drops its permit before waiting"),
        ("finish_height", ".store(true, AtomicOrdering::Release);", ".store(false, AtomicOrdering::Release);", "configured cleanup deadline"),
        ("network_exact_output_hash", "*envelope.exact_output_hash.get_or_init(|| HashOf::new(self))", "*envelope.exact_output_hash.get().expect(\"hash absent\")", "canonical whole-message hash"),
        ("network_exact_output_hash", "_ => HashOf::new(self),", "_ => unreachable!(),", "every variant"),
        ("wire_make_mut", "self.encoded = None;", "", "invalidates both bytes and exact identity"),
        ("wire_make_mut", "let _ = self.exact_output_hash.take();", "", "invalidates both bytes and exact identity"),
        ("body_retirement_job", "self.ensure_mutable()?;", "", "only after matching receipt context and height"),
        ("body_retirement_job", "kura_receipt.context_id() != self.context.id()", "false", "matching receipt context and height"),
        ("body_retirement_job", "|| kura_receipt.height() != self.context.height", "|| false", "matching receipt context and height"),
        ("body_retirement_job", "self.bound_directory.take()", "self.bound_directory.as_ref().cloned()", "consumes the exact bound directory"),
        ("body_retirement_execute", "self.directory.retire()", "Ok(())", "already-bound directory capability"),
        ("cleanup_execute", "job.bodies.execute()", "Ok::<(), V2BodyStoreError>(())", "executes durable body retirement"),
        ("cleanup_execute", "PostFinalityCleanupTarget::DurableBodies,", "PostFinalityCleanupTarget::CleanupWorker,", "records retained-file failures"),
    )

    def copy_source(relative: str, copy_root: Path, copied: set[str]) -> None:
        if relative in copied:
            return
        copied.add(relative)
        path = copy_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, path)
        for child in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative, ()):
            copy_source((Path(relative).parent / child).as_posix(), copy_root, copied)

    for index, (key, old, new, expected) in enumerate(mutations):
        copy_root = tmp_path / f"worker-ack-{index:02d}"
        copied: set[str] = set()
        for relative, _name, _context in module._WORKER_ACK_RECONCILIATION_OWNERS.values():
            copy_source(f"crates/iroha_core/src/sumeragi/{relative}", copy_root, copied)
        baseline = module._worker_ack_reconciled_source_fidelity_errors(copy_root)
        assert not baseline, (key, old, baseline)
        relative, name, context = module._WORKER_ACK_RECONCILIATION_OWNERS[key]
        path = copy_root / "crates/iroha_core/src/sumeragi" / relative
        brace_context = (module.rust_code_tokens(context),) if context else ()
        mutate_rust_item_source_in_context(module, path, name, brace_context, old, new)
        item = next(item for item in module.rust_items(path.read_text(), name) if item.brace_context == brace_context)
        digest = module._rust_item_token_sha256(item)
        seals = module._WORKER_ACK_RECONCILIATION_EXISTING_SEALS.get(key,
            (("_PRODUCTION_WORKER_ACK_RECONCILIATION_ITEM_SHA256", key),))
        restored = []
        try:
            for mapping_name, seal_key in seals:
                mapping = getattr(module, mapping_name)
                assert digest != mapping[seal_key], (key, old)
                restored.append((mapping, seal_key, mapping[seal_key]))
                mapping[seal_key] = digest
            errors = module._worker_ack_reconciled_source_fidelity_errors(copy_root)
            assert any(expected in error for error in errors), (key, old, errors)
            assert not any("exact reviewed token digest" in error for error in errors), (key, old, errors)
        finally:
            for mapping, seal_key, original in restored:
                mapping[seal_key] = original


def _ingress_effects_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Rehash real chunk, Apply exclusion and ticketless-regression owners."""

    module = load_checker()
    mappings = (
        module._PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256,
        module._APPLIED_HEIGHT_TICKETLESS_FINALITY_REGRESSION_TEST_SHA256,
        module._PRODUCTION_INGRESS_EFFECTS_RECONCILIATION_ITEM_SHA256,
    )
    originals = tuple(dict(mapping) for mapping in mappings)
    mutations = (
        ("effects::accept_payload_chunk_with_ingress_ownership", "wire::ConsensusMessageV2Payload::PayloadChunk(chunk),\n        ));", "wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),\n        ));", "same moved chunk"),
        ("effects::accept_payload_chunk_with_ingress_ownership", "if !ingress_ownership.validate_exact()", "if false", "same moved chunk"),
        ("effects::accept_payload_chunk_with_ingress_ownership", "|| !ingress_ownership.matches_message(&message)", "|| false", "same moved chunk"),
        ("effects::accept_payload_chunk_with_ingress_ownership", "|| !ingress_ownership.matches_semantic_origin(authenticated_sender)", "|| false", "same moved chunk"),
        ("effects::accept_payload_chunk_with_ingress_ownership", "let chunk = match message {", "let chunk = match foreign_message {", "same moved chunk"),
        ("effects::accept_payload_chunk_with_ingress_ownership", "self.accept_payload_chunk_inner(work_id, chunk, authenticated_sender, services)", "self.accept_payload_chunk_inner(work_id, foreign_chunk, authenticated_sender, services)", "same moved chunk"),
        ("worker::route_payload_chunk", "wire::ConsensusMessageV2Payload::PayloadChunk(chunk),\n        ));", "wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),\n        ));", "recovering and delivering the same chunk"),
        ("worker::route_payload_chunk", "|| !ingress_ownership.matches_message(&chunk_message)", "|| false", "recovering and delivering the same chunk"),
        ("worker::route_payload_chunk", "|| !ingress_ownership.matches_semantic_origin(&sender)", "|| false", "recovering and delivering the same chunk"),
        ("worker::route_payload_chunk", "let chunk = match chunk_message {", "let chunk = match foreign_message {", "recovering and delivering the same chunk"),
        ("worker::route_payload_chunk", "self.deliver_payload_chunk(executor, work_id, sender, chunk, ingress_ownership)", "self.deliver_payload_chunk(executor, work_id, sender, foreign_chunk, ingress_ownership)", "recovering and delivering the same chunk"),
        ("predecessor_remains_exact", "LifecycleDecisionApplySuccessorOutputModeV1::SameBatchSuffix", "LifecycleDecisionApplySuccessorOutputModeV1::AnySuffix", "restricted to the attested same-batch suffix"),
        ("predecessor_remains_exact", "&& self.finality_completion.is_none()", "&& self.finality_completion.is_some()", "no finality and the attested runtime ordinal"),
        ("predecessor_remains_exact", "&& self.pending_work() == self.pending_lifecycle_output_admissions.len()", "&& true", "exact census, no finality"),
        ("predecessor_remains_exact", "attestation.dispatch_key().lifecycle_ordinal(),", "0,", "attested runtime ordinal"),
        ("test_executor_owners_empty", "#[cfg(test)]", "#[cfg(any(test, not(test)))]", "unreviewed cfg/cfg_attr"),
        ("test_executor_owners_empty", "&& self.finality_completion.is_none()", "&& self.finality_completion.is_some()", "test-only executor emptiness"),
        ("test_executor_owners_empty", "&& self.finality_completion.is_none()", "&& { self.finality_completion = None; true }", "only runtime and lifecycle finality installation may assign"),
        ("test_executor_owners_empty", "&& self.runtime.queued_commands() == 0", "&& true", "test-only executor emptiness"),
        ("released_validate_preflight", "|| marker.terminal_no_successor_ordinal().is_none()", "|| false", "exact cleaned terminal Commit owner"),
        ("released_validate_preflight", "|| runtime_decision != self.protected_decision", "|| false", "exact cleaned terminal Commit owner"),
        ("released_validate_preflight", "|| self.pending_work() != 1", "|| self.pending_work() != 0", "exact cleaned terminal Commit owner"),
        ("released_validate_preflight", "|| self.finality_completion.is_some()", "|| self.finality_completion.is_none()", "exact cleaned terminal Commit owner"),
        ("released_validate_preflight", "|| !self.decision_body_drained", "|| false", "exact cleaned terminal Commit owner"),
        ("validate_body", "if pending.key() != key || !pending.exactly_matches_retry(&effect, &ownership) {", "if pending.key() != key {", "exact existing publication owner"),
        ("validate_body", "let projected = marker\n                .project_retry(&effect, &ownership)", "let projected = marker\n                .project_retry(&effect, &foreign_ownership)", "exact retry and incoming runtime binding"),
        ("validate_body", ".exact_pending_adapter_effect_binding(&effect)", ".exact_pending_adapter_effect_binding(&foreign_effect)", "exact retry and incoming runtime binding"),
        ("validate_body", "|| marker.terminal_no_successor_ordinal().is_none()", "|| false", "terminal no-successor Commit"),
        ("validate_body", "|| incoming_statement.phase() != Some(wire::GlobalPhase::Commit)", "|| false", "terminal no-successor Commit"),
        ("validate_body", "|| self.durable_validate_retry_seals.contains_key(&key)", "|| false", "without another replay authority"),
        ("validate_body", ".filter(|certificate| certificate.phase == wire::GlobalPhase::Commit)", ".filter(|certificate| certificate.phase == wire::GlobalPhase::Prepare)", "exact authenticated Commit certificate"),
        ("validate_body", "|| runtime_decision != Some(decision)", "|| false", "exact cached Decision"),
        ("validate_body", "|| projected.latest_effect != effect", "|| false", "all durable validation identities"),
        ("validate_body", "|| recovered_durable != durable", "|| false", "all durable validation identities"),
        ("validate_body", "|| projected.latest_statement.execution_commitment()\n                    != Some(certificate.execution_commitment)", "|| false", "all durable validation identities"),
        ("validate_body", "if self.decision_apply_dispatch_barrier_is_occupied()", "if false", "excludes competing Apply/finality"),
        ("validate_body", "self.ensure_pending_slot()?;\n            self.reconcile_decision_work(decision, true, services)?;", "self.reconcile_decision_work(decision, true, services)?;\n            self.ensure_pending_slot()?;", "before exact Decision cleanup"),
        ("validate_body", "self.reconcile_decision_work(decision, true, services)?;", "self.reconcile_decision_work(decision, false, services)?;", "before exact Decision cleanup"),
        ("validate_body", "owned.effect == effect && owned.ownership == ownership", "owned.effect == effect", "preserves only the current exact occurrence"),
        ("validate_body", "|| (self.retained_effect_batch.is_some() && !retained_is_current_occurrence)", "|| false", "rechecks finality"),
        ("validate_body", "ReleasedLifecycleValidatedMarkerSealPermitV1::new(),", "foreign_permit,", "seals the exact cached receipts"),
        ("validate_body", "                marker.published_pending.clone(),", "                incoming_pending.clone(),", "terminal predecessor"),
        ("validate_body", ".insert(key, projected)\n                .is_some()", ".insert(key, projected)\n                .is_none()", "restores only its projected terminal marker"),
        ("validate_body", ".replace(deferred)\n                    .is_none()", ".replace(deferred)\n                    .is_some()", "one deferred publication"),
        ("settle_released_validate", "self.preflight_pending_released_validate_apply_publication()", "Ok::<_, EffectExecutorError>(())", "checks executor ownership before coordinator capacity"),
        ("settle_released_validate", ") => return Ok(0),", ") => {},", "capacity deferral and preflight failure retain the pending owner before take"),
        ("settle_released_validate", ".replace(pending)\n                        .is_none()", ".replace(pending)\n                        .is_some()", "restores failed preparation"),
        ("settle_released_validate", "owner.publish_released_validate_apply(prepared)", "owner.publish_unchecked_apply(prepared)", "publishes durable authority before executor commit"),
        ("settle_released_validate", "self.commit_released_validate_apply_publication(key, ordinal, authority);", "self.commit_released_validate_apply_publication(key, 0, authority);", "only the published exact authority"),
        ("settle_released_validate", "return Err(self.close(\n                    EffectExecutorError::Contract(format!(\n                        \"released Validate Apply lifecycle publication failed: {error}\"\n                    )),\n                    services,\n                ));", "return Err(EffectExecutorError::Contract(format!(\n                    \"released Validate Apply lifecycle publication failed: {error}\"\n                )));", "only the published exact authority"),
        ("commit_released_validate", ".remove(&key)\n            .expect(\"preflight retained the exact terminal Validate marker\")", ".get(&key).cloned()\n            .expect(\"preflight retained the exact terminal Validate marker\")", "consumes its terminal marker"),
        ("commit_released_validate", "self.live_lifecycle_decision_apply = Some(LiveLifecycleDecisionApplyOwnerV1 {", "self.live_lifecycle_decision_apply = Some(ArbitraryApplyOwner {", "typed live Apply owner"),
        ("commit_released_validate", "assert_eq!(dispatch_key.lifecycle_ordinal(), ordinal);", "assert_eq!(dispatch_key.lifecycle_ordinal(), 0);", "exact published ordinal, Decision and durable receipts"),
        ("commit_released_validate", "assert_eq!(self.validated_bodies.get(&key), Some(&validated_receipt));", "assert!(self.validated_bodies.contains_key(&key));", "exact published ordinal, Decision and durable receipts"),
        ("coordinator_released_validate_preflight", "|| coordinator.lifecycle_ordinal_authority.is_none()", "|| false", "live durable ordinal authority"),
        ("coordinator_released_validate_preflight", "|| effect_used >= effect_limit", "|| false", "bounded effect and record capacity"),
        ("coordinator_released_validate_preflight", "|| !super::schema::has_lifecycle_record_capacity(coordinator.records.len(), 1)", "|| false", "bounded effect and record capacity"),
        ("coordinator_released_validate_publish", ".project_apply_candidate(&SealedValidateApplyProjectionPermit::new(), verified)", ".project_apply_candidate(&foreign_permit, verified)", "authenticated independent Ready Apply body frame"),
        ("coordinator_released_validate_publish", "|| candidate.stage.predecessor_scope() != PredecessorScope::Independent", "|| false", "authenticated independent Ready Apply body frame"),
        ("coordinator_released_validate_publish", "&& staged.records.len() == records_before.saturating_add(1)", "&& true", "exactly one fully indexed durable record"),
        ("coordinator_released_validate_publish", "metadata.continuation == DurableContinuation::None", "true", "exactly one fully indexed durable record"),
        ("coordinator_released_validate_publish", "if !prepared.registry_work_matches(owner, ordinal, slot, digest) {", "if false {", "mismatched concrete registry address"),
        ("coordinator_released_validate_publish", ".persist_exact_staged_successor_with_ordinal_reservation(&staged, &ordinal_reservation)", ".persist_unchecked_staged_successor(&staged, &ordinal_reservation)", "fsyncs before any owner installation"),
        ("coordinator_released_validate_publish", "let reconciliation = reservation.install_after_ledger_fsync();\n        *coordinator = staged;", "*coordinator = staged;\n        let reconciliation = reservation.install_after_ledger_fsync();", "fsyncs before any owner installation"),
        ("coordinator_released_validate_publish", "            coordinator.fault = Some(CoordinatorFault::DurabilityFailure);", "            coordinator.fault = None;", "fsyncs before any owner installation"),
        ("register_outbound_payload", "self.sign_payload_chunks(payload, sender)?", "self.sign_payload_chunks(payload, sender).unwrap()", "signed canonical frames"),
        ("register_outbound_payload", "Self::preencode_v2_network_message(wire::ConsensusMessageV2::new(", "Self::unencoded_v2_network_message(wire::ConsensusMessageV2::new(", "signed canonical frames"),
        ("register_outbound_payload", "if self.proposal_work_retired {", "if false {", "excludes retired proposal work"),
        ("register_outbound_payload", "output_guard.begin_fail_stop_operation()", "output_guard.begin_unchecked_operation()", "fail-stop output guard"),
        ("register_outbound_payload", "if owner != self.active_tag || payload.manifest().round != expected_round {", "if payload.manifest().round != expected_round {", "matching incarnation and cached manifest"),
        ("register_outbound_payload", "if !existing.owns_manifest(owner, payload.manifest()) {", "if false {", "matching incarnation and cached manifest"),
        ("register_outbound_payload", ".retain(|hash, _| *hash == manifest_hash);", ".retain(|_, _| true);", "matching incarnation and cached manifest"),
        ("register_outbound_payload", "self.outbound_chunks.insert(manifest_hash, retained);\n        operation.complete();", "operation.complete();\n        self.outbound_chunks.insert(manifest_hash, retained);", "before completing output ownership"),
        ("production_exact_output_observes_finality_only_after_state_commit", "wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(", "wire::ExecutionCommitment::without_topups_or_merge_carrier(", "current empty top-up/merge constructor"),
        ("production_exact_output_observes_finality_only_after_state_commit", "assert!(pending.applied_height_finality.is_none());", "assert!(pending.applied_height_finality.is_some());", "Kura-only fixture leaves both pending output"),
        ("production_exact_output_observes_finality_only_after_state_commit", "state_block.commit().expect(\"commit synthetic State block\");", "drop(state_block);", "actual committed State boundary"),
        ("production_exact_output_observes_finality_only_after_state_commit", "assert_eq!(pending.applied_height_finality.as_ref(), Some(&artifact));", "assert!(pending.applied_height_finality.is_some());", "exact artifact adoption after State commit"),
        ("applied_height_finality_releases_only_covered_ticketless_payload_chunks", ".get(&HashOf::new(&manifest))", ".get(&HashOf::new(&foreign_manifest))", "registered preencoded manifest-bound frames"),
        ("applied_height_finality_releases_only_covered_ticketless_payload_chunks", "assert_eq!(ticketless_attempts, chunk_count);", "assert_eq!(ticketless_attempts, 1);", "discharges every covered frame"),
        ("applied_height_finality_releases_only_covered_ticketless_payload_chunks", "assert!(ticketed.is_pending(), \"ticketed payload chunks stay owned\");", "assert!(!ticketed.is_pending(), \"ticketed payload chunks stay owned\");", "genuine ticketed negative control"),
        ("applied_height_finality_releases_only_covered_ticketless_payload_chunks", "assert!(\n        uncovered.is_pending(),", "assert!(\n        !uncovered.is_pending(),", "uncovered-scope negative control"),
    )

    def copy_source(relative: str, copy_root: Path, copied: set[str]) -> None:
        if relative in copied:
            return
        copied.add(relative)
        path = copy_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, path)
        for child in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative, ()):
            copy_source((Path(relative).parent / child).as_posix(), copy_root, copied)

    try:
        for index, (key, old, new, expected) in enumerate(mutations):
            copy_root = tmp_path / f"ingress-effects-{index:02d}"
            copied: set[str] = set()
            for relative, _name, _context, _attributes in module._INGRESS_EFFECTS_RECONCILIATION_OWNERS.values():
                copy_source(f"crates/iroha_core/src/sumeragi/{relative}", copy_root, copied)
            baseline = module._ingress_effects_source_fidelity_errors(copy_root)
            assert not baseline, (key, old, baseline)
            relative, name, context, _attributes = module._INGRESS_EFFECTS_RECONCILIATION_OWNERS[key]
            path = copy_root / "crates/iroha_core/src/sumeragi" / relative
            brace_context = (module.rust_code_tokens(context),) if context else ()
            if old.startswith("#[cfg("):
                source = path.read_text()
                item = next(item for item in module.rust_items(source, name) if item.brace_context == brace_context)
                item_start = source.index(item.source)
                attribute_start = source.rfind(old, 0, item_start)
                assert attribute_start >= 0 and not source[attribute_start + len(old):item_start].strip()
                path.write_text(source[:attribute_start] + new + source[attribute_start + len(old):])
                # Attribute context is sealed independently of item-body hashes.
                # A harmless statement also changes the entire item digest so
                # this control proves that refreshing it cannot hide cfg drift.
                mutate_rust_item_source_in_context(module, path, name, brace_context,
                    "self.pending_work() == 0", "let _reviewed = (); self.pending_work() == 0")
            else:
                mutate_rust_item_source_in_context(module, path, name, brace_context, old, new)
            item = next(item for item in module.rust_items(path.read_text(), name) if item.brace_context == brace_context)
            mapping = next(mapping for mapping in mappings if key in mapping)
            digest = module._rust_item_token_sha256(item)
            assert digest != mapping[key], (key, old)
            mapping[key] = digest
            errors = module._ingress_effects_source_fidelity_errors(copy_root)
            assert any(expected in error for error in errors), (key, old, errors)
            assert not any("exact reviewed token digest" in error for error in errors), (key, old, errors)
            for mapping, original in zip(mappings, originals, strict=True):
                mapping.clear()
                mapping.update(original)
    finally:
        for mapping, original in zip(mappings, originals, strict=True):
            mapping.clear()
            mapping.update(original)


def _decided_body_serve_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Rehash each real worker-chain owner after an independently rejected mutation."""

    module = load_checker()
    original = dict(module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256)
    mutations = (
        ("commit_certified_serve", "let ingress_ownership = self.take_bound_leader_wire()?;", "let ingress_ownership = self.take_bound_leader_wire().unwrap();", "bound occurrence and authenticated hop"),
        ("commit_certified_serve", "let authenticated_via = inbound.via().clone();", "let authenticated_via = inbound.sender().clone();", "bound occurrence and authenticated hop"),
        ("commit_certified_serve", ".validate_version()", ".accept_any_version()", "validates wire version"),
        ("commit_certified_serve", "let BlockMessage::V2(message) = message else {", "let BlockMessage::Unverified(message) = message else {", "authorized message family"),
        ("commit_certified_serve", "let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = message.payload else {", "let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) = message.payload else {", "authorized request payload"),
        ("commit_certified_serve", "if !scope.permits_height(request.round.height, self.executor.context().height) {", "if false {", "exact height, subject and reply target"),
        ("commit_certified_serve", "if !scope.permits_subject(request.subject, self.decided_subject) {", "if false {", "exact height, subject and reply target"),
        ("commit_certified_serve", "if reply_routes.semantic_target() != &sender {", "if false {", "exact height, subject and reply target"),
        ("commit_certified_serve", "            request,\n            sender,\n            authenticated_via,", "            request,\n            authenticated_via.clone(),\n            authenticated_via,", "retains queued ownership"),
        ("commit_certified_serve", "            authenticated_via,\n            reply_routes,", "            sender.clone(),\n            reply_routes,", "retains queued ownership"),
        ("commit_certified_serve", "            reply_routes,\n            ingress_ownership,", "            reply_routes.retain_active(),\n            ingress_ownership,", "retains queued ownership"),
        ("commit_certified_serve", "            ingress_ownership,\n        );", "            terminal_ownership,\n        );", "retains queued ownership"),
        ("commit_certified_serve", "self.block_sync_server.try_enqueue_historical_body(task)", "Ok(HistoricalBodyServeAdmission::Queued)", "retains queued ownership"),
        ("commit_certified_serve", "Ok(HistoricalBodyServeAdmission::Queued) => {}", "Ok(HistoricalBodyServeAdmission::Queued) => { mark_leader_wire_volatile(self.receiver, &terminal_ownership)?; }", "retains queued ownership"),
        ("commit_certified_serve", "HistoricalBodyServeAdmission::RateLimited | HistoricalBodyServeAdmission::Busy", "HistoricalBodyServeAdmission::RateLimited", "retains queued ownership"),
        ("commit_certified_serve", '"retired certified body request at bounded terminal-recovery worker admission"\n                );\n                mark_leader_wire_volatile(self.receiver, &terminal_ownership)?;', '"retired certified body request at bounded terminal-recovery worker admission"\n                );', "retains queued ownership"),
        ("commit_certified_serve", "Err(error) if is_remote_block_sync_rejection(&error) => {", "Err(error) if true => {", "retains queued ownership"),
        ("commit_certified_serve", '"rejected certified body request during terminal recovery"\n                );\n                mark_leader_wire_volatile(self.receiver, &terminal_ownership)?;', '"rejected certified body request during terminal recovery"\n                );', "retains queued ownership"),
        ("commit_certified_serve", "Err(error) => return Err(error.into()),", "Err(_error) => {},", "retains queued ownership"),
        ("commit_certified_serve", "let task = HistoricalBodyServeTask::from_bound_ingress(", "let _ = self.block_sync_server.serve_historical_body(self.kura, request.clone(), &sender, self.local_key);\n        let task = HistoricalBodyServeTask::from_bound_ingress(", "only the prepared worker seam"),
        ("bind_leader_wire", "|| !ingress_ownership.matches_message(inbound.message())", "|| false", "complete fair-ingress ownership"),
        ("bind_leader_wire", ".bind_leader_wire_runtime_ownership(&mut ingress_ownership)", ".skip_leader_wire_runtime_ownership(&mut ingress_ownership)", "checked runtime-bound ingress carrier"),
        ("permits_height", "Self::Historical => request < active,", "Self::Historical => request <= active,", "exact current versus historical height"),
        ("permits_height", "Self::Current => request == active,", "Self::Current => request <= active,", "exact current versus historical height"),
        ("permits_subject", "Self::Current => request == decided,", "Self::Current => true,", "exact Decision subject"),
        ("task_from_bound_ingress", "if request.requester != recipient", "if false", "authenticates exact request, hop, full routes and ingress"),
        ("task_from_bound_ingress", "|| reply_routes.semantic_target() != &recipient", "|| false", "authenticates exact request, hop, full routes and ingress"),
        ("task_from_bound_ingress", ".any(|route| route.is_authenticated_via(&authenticated_via))", ".any(|route| route.is_active())", "authenticates exact request, hop, full routes and ingress"),
        ("task_from_bound_ingress", "|| !ingress_ownership.matches_message(&exact_message)", "|| false", "authenticates exact request, hop, full routes and ingress"),
        ("task_from_bound_ingress", "|| !ingress_ownership.matches_reply_routes(Some(&reply_routes))", "|| false", "authenticates exact request, hop, full routes and ingress"),
        ("task_from_bound_ingress", "authenticate_certified_body_request_identity(&request, &recipient)?;", "", "validates the signature before constructing"),
        ("task_from_bound_ingress", "HistoricalBodyAdmissionPlan::from_reply_routes(&reply_routes)?", "HistoricalBodyAdmissionPlan::from_reply_routes(&reply_routes).unwrap()", "validates the signature before constructing"),
        ("server_enqueue", ".try_enqueue(task)", ".try_enqueue_unbounded(task)", "requires the installed service"),
        ("worker_spawn", "let (completion_tx, completion_rx) = mpsc::sync_channel(queue_capacity);", "let (completion_tx, completion_rx) = mpsc::channel();", "validated finite geometry"),
        ("worker_spawn", "limits.validate()?;", "", "validated finite geometry"),
        ("worker_try_enqueue", "if self.deferred_prepared.is_some() {", "if false {", "reserves budgets before nonblocking send"),
        ("worker_try_enqueue", "if !self.admission.try_reserve(&task, now)? {", "if false {", "reserves budgets before nonblocking send"),
        ("worker_try_enqueue", "self.task_tx.try_send(task)", "self.task_tx.send(task)", "reserves budgets before nonblocking send"),
        ("worker_try_enqueue", "Err(TrySendError::Full(task)) => {\n                self.admission.release(&task)?;", "Err(TrySendError::Full(task)) => {", "releases exact failed charges"),
        ("worker_try_enqueue", "Err(TrySendError::Disconnected(task)) => {\n                self.admission.release(&task)?;", "Err(TrySendError::Disconnected(task)) => {", "releases exact failed charges"),
        ("worker_try_recv", "self.deferred_prepared.take()", "self.deferred_prepared.clone()", "prioritizes the exact retry"),
        ("worker_try_recv", "self.admission.release(completion.task())?;", "", "releases each worker charge once"),
        ("worker_defer_prepared", "if self.deferred_prepared.is_some() {", "if false {", "single retained retry owner"),
        ("worker_has_pending", "self.deferred_prepared.is_some() || self.admission.outstanding != 0", "self.admission.outstanding != 0", "retry or outstanding ingress exists"),
        ("historical_body_worker", "&task.request,\n            &task.recipient,", "&task.request,\n            &task.authenticated_via,", "exact authenticated task through every completion"),
        ("cache_serve", "authenticate_certified_body_request_identity(request, authenticated_requester)?;", "", "authenticates the signed requester even on cache hits"),
        ("cache_serve", "build_historical_body_response(", "build_unverified_body_response(", "canonical authenticated Kura response"),
        ("cache_prepare", "let _ = message.exact_output_hash();", "", "warms the exact output hash"),
        ("cache_prepare", "BlockMessageWire::try_preencoded(", "BlockMessageWire::new(", "preencodes and warms"),
        ("proof_mint", "source_subject: request.subject,", "source_subject: response.manifest.subject,", "exact request, round, subject, responder and output hash"),
        ("validated_cached_exact_output_hash", "|| response.manifest.subject != request.subject", "|| false", "foreign request, source round or subject"),
        ("validated_cached_exact_output_hash", "message.cached_exact_output_hash().ok_or_else(", "Some(message.exact_output_hash()).ok_or_else(", "requires the worker-warmed exact hash"),
        ("proof_covers_message", "if &self.network_id != expected_network_id {", "if false {", "preserves its exact network"),
        ("proof_covers_message", "&& response.manifest.round == self.source_round", "&& true", "every immutable output identity field"),
        ("proof_covers_message", "&& message.cached_exact_output_hash() == Some(self.exact_output_hash)", "&& true", "every immutable output identity field"),
        ("settle_completion", ".begin_fail_stop_operation()", ".acquire()", "posts under fail-stop guard"),
        ("settle_completion", "operation.permit(),", "foreign_permit,", "posts under fail-stop guard"),
        ("settle_completion", "block_sync_server.defer_prepared_historical_body_output(prepared)", "Ok::<_, V2BlockSyncError>(())", "retains an exact rejected output"),
        ("settle_completion", "drop(operation);\n                        return Err(error.into());", "operation.complete();\n                        return Err(error.into());", "posts under fail-stop guard"),
        ("settle_completion", "drop(operation);\n                    return Err(V2BlockSyncError::ResponsePost(error).into());", "operation.complete();\n                    return Err(V2BlockSyncError::ResponsePost(error).into());", "posts under fail-stop guard"),
        ("settle_completion", "HistoricalBodyServeCompletion::NoResponse(task) => {\n            mark_leader_wire_volatile(receiver, task.ingress_ownership())?;", "HistoricalBodyServeCompletion::NoResponse(task) => {", "retires exact remote/no-response ingress"),
        ("settle_completion", "if is_remote_block_sync_rejection(&error) =>", "if true =>", "propagates local failure"),
        ("settle_completion", "HistoricalBodyServeCompletion::Failed(_task, error) => return Err(error.into()),", "HistoricalBodyServeCompletion::Failed(_task, _error) => {},", "propagates local failure"),
        ("post_prepared", "|| !ingress_ownership.matches_reply_routes(Some(&reply_routes))", "|| false", "authenticates ingress, full routes"),
        ("post_prepared", "|| proof.network_id() != self.context.network_id", "|| false", "non-future local durable proof"),
        ("post_prepared", "|| proof.source_round().height > self.context.height", "|| proof.source_round().height < self.context.height", "non-future local durable proof"),
        ("post_prepared", "|| proof.responder() != &self.local_peer", "|| false", "non-future local durable proof"),
        ("post_prepared", "|| !proof.covers_message_in_network(&self.context.network_id, &message)", "|| false", "non-future local durable proof"),
        ("post_prepared", "ExactOutputRolloverClaim::DurableCertifiedBodyResponse", "ExactOutputRolloverClaim::Volatile", "typed durable claim before guarded ownership transfer"),
        ("post_prepared", "durable_history_source_covers(", "skip_durable_history_source_covers(", "typed durable claim before guarded ownership transfer"),
        ("post_prepared", "Some(ingress_ownership),", "None,", "full routes and exact ingress"),
        ("post_prepared", "super::v2_block_sync::PreparedHistoricalBodyPostOutcome::SourceRetained(retry)", "super::v2_block_sync::PreparedHistoricalBodyPostOutcome::Posted", "exact source on output capacity rejection"),
    )

    def copy_source(relative: str, copy_root: Path, copied: set[str]) -> None:
        if relative in copied:
            return
        copied.add(relative)
        path = copy_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, path)
        for child in module._REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative, ()):
            copy_source((Path(relative).parent / child).as_posix(), copy_root, copied)

    try:
        for index, (key, old, new, expected) in enumerate(mutations):
            copy_root = tmp_path / f"decided-body-{index:02d}"
            copied: set[str] = set()
            for relative, _name, _context in module._DECIDED_BODY_SERVE_SOURCE_OWNERS.values():
                copy_source(f"crates/iroha_core/src/sumeragi/{relative}", copy_root, copied)
            baseline = module._decided_body_serve_source_fidelity_errors(copy_root)
            assert not baseline, (key, old, baseline)
            relative, name, context = module._DECIDED_BODY_SERVE_SOURCE_OWNERS[key]
            path = copy_root / "crates/iroha_core/src/sumeragi" / relative
            brace_context = (module.rust_code_tokens(context),) if context else ()
            mutate_rust_item_source_in_context(module, path, name, brace_context, old, new)
            item = next(item for item in module.rust_items(path.read_text(), name) if item.brace_context == brace_context)
            digest = module._rust_item_token_sha256(item)
            assert digest != original[key], (key, old)
            module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256[key] = digest
            errors = module._decided_body_serve_source_fidelity_errors(copy_root)
            assert any(expected in error for error in errors), (key, old, errors)
            assert not any("exact reviewed token digest" in error for error in errors), (key, old, errors)
            module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256.clear()
            module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256.update(original)
    finally:
        module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256.clear()
        module._PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256.update(original)


def _ordinary_ingress_consumer_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Exercise each reviewed ordinary-tail delta after rebinding both owner seals."""

    module = load_checker()
    relative = Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs")
    name = "consume_prepared_dequeued_v2_ingress"
    original_scalar = module._PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256
    original_map = dict(module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256)
    mutations = (
        ("propagate lane ingress failure", "executor.current_tag().view(),\n            )?;",
         "executor.current_tag().view(),\n            );", "lane ingress failure and shared recovery"),
        ("shared recovery service", "service_historical_recovery_tick(lane_work, services)?",
         "lane_work.service_next_historical_recovery()?", "lane ingress failure and shared recovery"),
        ("propagate recovery failure", "service_historical_recovery_tick(lane_work, services)?",
         "service_historical_recovery_tick(lane_work, services)", "lane ingress failure and shared recovery"),
        ("preserve authenticated hop", "let authenticated_via = inbound.via().clone();",
         "let authenticated_via = inbound.sender().clone();", "authenticated hop before consuming"),
        ("authenticate route set", "if !ingress_ownership.matches_reply_routes(reply_routes.as_ref()) {",
         "if false {", "authenticated hop before consuming"),
        ("strict historical height", "if request.round.height < executor.context().height {",
         "if request.round.height <= executor.context().height {", "bounded historical worker handoff"),
        ("transfer exact requester", "                    request,\n                    sender,\n                    authenticated_via,",
         "                    request,\n                    authenticated_via.clone(),\n                    authenticated_via,", "bounded historical worker handoff"),
        ("transfer authenticated hop", "                    authenticated_via,\n                    reply_routes,",
         "                    sender.clone(),\n                    reply_routes,", "bounded historical worker handoff"),
        ("preserve complete route history", "                    reply_routes,\n                    ingress_ownership,\n                );",
         "                    reply_routes.retain_active(),\n                    ingress_ownership,\n                );", "bounded historical worker handoff"),
        ("preserve exact worker ingress", "                    ingress_ownership,\n                );\n                match task.and_then",
         "                    terminal_ownership,\n                );\n                match task.and_then", "bounded historical worker handoff"),
        ("installed bounded worker", "block_sync_server.try_enqueue_historical_body(task)",
         "Ok(HistoricalBodyServeAdmission::Queued)", "bounded historical worker handoff"),
        ("keep queued ownership alive", "Ok(HistoricalBodyServeAdmission::Queued) => {}",
         "Ok(HistoricalBodyServeAdmission::Queued) => { mark_leader_wire_volatile(receiver, &terminal_ownership)?; }", "bounded historical worker handoff"),
        ("retire both capacity rejections", "HistoricalBodyServeAdmission::RateLimited\n                        | HistoricalBodyServeAdmission::Busy",
         "HistoricalBodyServeAdmission::RateLimited", "bounded historical worker handoff"),
        ("capacity rejection retires exact owner", "worker admission gate\"\n                        );\n                        mark_leader_wire_volatile(receiver, &terminal_ownership)?;",
         "worker admission gate\"\n                        );", "bounded historical worker handoff"),
        ("remote rejection classification", "Err(error) if is_remote_block_sync_rejection(&error) => {",
         "Err(error) if true => {", "bounded historical worker handoff"),
        ("remote rejection retires exact owner", '"rejected historical certified body request");\n                        mark_leader_wire_volatile(receiver, &terminal_ownership)?;',
         '"rejected historical certified body request");', "bounded historical worker handoff"),
        ("local worker failure propagates", "Err(error) => return Err(error.into()),",
         "Err(_error) => {},", "bounded historical worker handoff"),
        ("current-height rejection remains separate", "} else if request.round.height == executor.context().height {",
         "} else if request.round.height >= executor.context().height {", "bounded historical worker handoff"),
        ("synchronous body reconstruction stays absent", "let task = HistoricalBodyServeTask::from_bound_ingress(",
         "let _ = block_sync_server.serve_historical_body(kura, request.clone(), &sender, local_key);\n                let task = HistoricalBodyServeTask::from_bound_ingress(", "historical body work must remain off the ordinary actor"),
        ("retired manifest path stays absent", "wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {",
         "wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => { drop(manifest); }\n        wire::ConsensusMessageV2Payload::PayloadChunk(chunk) => {", "retired standalone manifest ingress"),
        ("CommitQC guarded reconstruction", "|| block_sync_server.serve(kura, request, &sender, local_key),",
         "|| Ok(None),", "CommitQC discovery must remain synchronous under its output guard"),
        ("CommitQC complete reply routes", "services.post_durable_history_response_on_reply_routes_with_permit(",
         "services.post_durable_history_response_with_permit(", "historical global responses preserve the complete prevalidated route set"),
        ("CommitQC exact terminal", "|| mark_leader_wire_volatile(receiver, &terminal_ownership),",
         "|| Ok(()),", "CommitQC discovery must remain synchronous under its output guard"),
        ("reply target validation", "if reply_routes.semantic_target() != &sender {\n                iroha_logger::debug!(\n                    %sender,\n                    \"rejected certified body request with mismatched reply target\"",
         "if false {\n                iroha_logger::debug!(\n                    %sender,\n                    \"rejected certified body request with mismatched reply target\"", "historical response route sets must match"),
    )
    try:
        for index, (case, old, new, expected) in enumerate(mutations):
            copy_root = tmp_path / f"ordinary-{index:02d}"
            path = copy_root / relative
            path.parent.mkdir(parents=True)
            shutil.copyfile(ROOT_DIR / relative, path)
            baseline = module._ordinary_ingress_consumer_source_fidelity_errors(copy_root)
            assert not baseline, (case, baseline)
            mutate_rust_item_source_in_context(module, path, name, (), old, new)
            source = path.read_text(encoding="utf-8")
            item = next(item for item in module.rust_items(source, name) if item.brace_context == ())
            digest = module._rust_item_token_sha256(item)
            assert digest != original_scalar, case
            module._PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256 = digest
            module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256[name] = digest
            errors = module._ordinary_ingress_consumer_source_fidelity_errors(copy_root)
            assert any(expected in error for error in errors), (case, errors)
            assert not any("exact reviewed token digest" in error for error in errors), (case, errors)
            module._PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256 = original_scalar
            module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256.clear()
            module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256.update(original_map)
    finally:
        module._PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256 = original_scalar
        module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256.clear()
        module._PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256.update(original_map)


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            'adapter.limits.session_capacity = NonZeroUsize::new(1).expect("one exact recovery slot");',
            'adapter.limits.session_capacity = NonZeroUsize::new(2).expect("one exact recovery slot");',
            "set actual capacity one before canonical ownership arrives",
        ),
        (
            "adapter.lane_sessions = LaneBlockSessionCache::new(1);",
            "adapter.lane_sessions = LaneBlockSessionCache::new(2);",
            "set actual capacity one before canonical ownership arrives",
        ),
        (
            "for _ in 0..2 {",
            "for _ in 0..0 {",
            "retain incomplete certificate progress in the active predecessor",
        ),
        (
            '.persist_anchored_sessions()\n                    .expect("repeat persistence must not charge an exact cached recovery twice")',
            '.persist_anchored_sessions().or(Ok::<_, V2LaneWorkError>(0))\n                    .expect("repeat persistence must not charge an exact cached recovery twice")',
            "retain incomplete certificate progress in the active predecessor",
        ),
        (
            '.hydrate_canonical_lane_artifacts()\n                .expect("direct hydration remains idempotent at its exact capacity");',
            '.hydrate_canonical_lane_artifacts().ok();',
            "retain incomplete certificate progress in the active predecessor",
        ),
        (
            "assert_eq!(adapter.lane_sessions, exact_recovered_cache);",
            "assert!(adapter.lane_sessions.len() <= 2);",
            "retain incomplete certificate progress in the active predecessor",
        ),
        (
            "assert!(!adapter.output_guard.restart_required());",
            "let _ = adapter.output_guard.restart_required();",
            "retain incomplete certificate progress in the active predecessor",
        ),
        (
            "Some(&proposal.descriptor.lane_block_height)",
            "None",
            "retain incomplete certificate progress in the active predecessor",
        ),
    ),
)
def test_late_lane_recovery_runtime_mutations_survive_digest_refresh(
    tmp_path: Path, old: str, new: str, expected_error: str,
) -> None:
    """The capacity-one regression must exercise real repeated recovery and fail-stop."""

    module = load_checker()
    relative = Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs")
    path = tmp_path / relative
    path.parent.mkdir(parents=True)
    shutil.copyfile(ROOT_DIR / relative, path)
    source = path.read_text(encoding="utf-8")
    name = "globally_applied_lane_body_without_certificate_remains_recoverable"
    errors = []
    item = module._require_rust_item(path, source, name, errors)
    assert item is not None
    module._require_rust_item_token_sha256(
        path, item, module._LATE_LANE_RECOVERY_TEST_SHA256, name, errors
    )
    module._require_late_lane_recovery_runtime_source_contracts(path, item, errors)
    assert not errors, errors
    mutate_rust_item_source_in_context(
        module, path, name, item.brace_context, old, new
    )
    errors = []
    mutated = module._require_rust_item(path, path.read_text(encoding="utf-8"), name, errors)
    assert mutated is not None
    original_digest = module._LATE_LANE_RECOVERY_TEST_SHA256
    module._LATE_LANE_RECOVERY_TEST_SHA256 = module._rust_item_token_sha256(mutated)
    try:
        module._require_rust_item_token_sha256(
            path, mutated, module._LATE_LANE_RECOVERY_TEST_SHA256, name, errors
        )
        module._require_late_lane_recovery_runtime_source_contracts(path, mutated, errors)
    finally:
        module._LATE_LANE_RECOVERY_TEST_SHA256 = original_digest
    assert any(expected_error in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "insert_recovered_proposals",
            "self.preflight_trusted_proposal_replacement(proposal)?;",
            "let _ = proposal;",
            "preflight every input against original quorum evidence",
        ),
        (
            "insert_recovered_proposals",
            "Some(previous) if previous != proposal",
            "Some(previous) if !previous.same_consensus_identity(proposal)",
            "preflight every input against original quorum evidence",
        ),
        (
            "insert_recovered_proposals",
            "None => ordered_required.push(proposal),",
            "None => {},",
            "preserve exact first-occurrence caller order",
        ),
        (
            "insert_recovered_proposals",
            ".is_some_and(|previous| previous != key.proposal_hash)",
            ".is_some_and(|previous| previous == key.proposal_hash)",
            "bound the unique consistent required union before cloning",
        ),
        (
            "insert_recovered_proposals",
            "if required.len() > self.capacity",
            "if required.len() > self.capacity.saturating_add(self.sessions.len())",
            "bound the unique consistent required union before cloning",
        ),
        (
            "insert_recovered_proposals",
            "if required.len() > self.capacity",
            "if required.len() >= self.capacity",
            "bound the unique consistent required union before cloning",
        ),
        (
            "insert_recovered_proposals",
            "move_preflight_after_insertion",
            "",
            "preflight every input against original quorum evidence",
        ),
        (
            "insert_recovered_proposals",
            "next.touch(key);",
            "let _ = key;",
            "touch required survivors before insertion",
        ),
        (
            "insert_recovered_proposals",
            "for proposal in ordered_required {",
            "for proposal in required.values() {",
            "use trusted insertion in caller order",
        ),
        (
            "insert_recovered_proposals",
            "next.insert_trusted_proposal_replacing_uncommitted_conflict(proposal.clone())?;",
            "next.insert_proposal(proposal.clone())?;",
            "use trusted insertion in caller order",
        ),
        (
            "insert_recovered_proposals",
            "next.touch(LaneBlockSessionKey::from_proposal(proposal));",
            "let _ = LaneBlockSessionKey::from_proposal(proposal);",
            "use trusted insertion in caller order",
        ),
        (
            "insert_recovered_proposals",
            "next.sessions\n                .get(key)\n                .and_then(|session| session.proposal.as_ref())\n                != Some(*proposal)",
            "!next.sessions.get(key).and_then(|session| session.proposal.as_ref()).is_some_and(|retained| retained.same_consensus_identity(proposal))",
            "verify the full exact retained set before atomic publication",
        ),
        (
            "insert_recovered_proposals",
            "if required.iter().any(|(key, proposal)| {",
            "if false && required.iter().any(|(key, proposal)| {",
            "verify the full exact retained set before atomic publication",
        ),
        (
            "insert_recovered_proposals",
            "if required.iter().any(|(key, proposal)| {",
            "*self = next.clone();\n        if required.iter().any(|(key, proposal)| {",
            "verify the full exact retained set before atomic publication",
        ),
        (
            "insert_recovered_proposals",
            "let mut next = self.clone();",
            "let _extra_snapshot = self.clone();\n        let mut next = self.clone();",
            "stage exactly one cache clone",
        ),
        (
            "preflight_trusted_proposal_replacement",
            "validate_lane_block_proposal(proposal).map_err(LaneBlockSessionError::InvalidProposal)?;",
            "let _ = proposal;",
            "validate the proposal and protect any original same-slot quorum",
        ),
        (
            "preflight_trusted_proposal_replacement",
            "self\n            .sessions\n            .range(first..=last)",
            "self.sessions.range(key..=key)",
            "protect any original same-slot quorum including proposal-less evidence",
        ),
        (
            "preflight_trusted_proposal_replacement",
            "session_has_quorum_certificate(session)",
            "session.commit_qc.is_some()",
            "protect any original same-slot quorum including proposal-less evidence",
        ),
        (
            "preflight_trusted_proposal_replacement",
            "retained_key.proposal_hash != key.proposal_hash\n                    && session_has_quorum_certificate(session)",
            "retained_key.proposal_hash != key.proposal_hash && session.proposal.is_some() && session_has_quorum_certificate(session)",
            "protect any original same-slot quorum including proposal-less evidence",
        ),
        (
            "insert_trusted_proposal_replacing_uncommitted_conflict",
            "self.preflight_trusted_proposal_replacement(&proposal)?;",
            "let _ = &proposal;",
            "share the original-quorum preflight before every mutation",
        ),
    ),
)
def test_lane_recovery_cache_mutations_survive_digest_refresh(
    tmp_path: Path, item_name: str, old: str, new: str, expected_error: str,
) -> None:
    """A refreshed target seal cannot hide weakened transactional recovery."""

    module = load_checker()
    relative = Path("crates/iroha_core/src/lane_consensus.rs")
    cache_path = tmp_path / relative
    cache_path.parent.mkdir(parents=True)
    shutil.copyfile(ROOT_DIR / relative, cache_path)
    baseline_errors = _lane_recovery_cache_owner_contract_errors(module, cache_path)
    assert not baseline_errors, baseline_errors
    context = (("impl", "LaneBlockSessionCache"),)
    if old == "move_preflight_after_insertion":
        source = cache_path.read_text(encoding="utf-8")
        item = next(
            item for item in module.rust_items(source, item_name)
            if item.brace_context == context
        )
        old = item.source
        new = old.replace(
            "self.preflight_trusted_proposal_replacement(proposal)?;",
            "let _ = proposal;", 1,
        ).replace(
            "next.insert_trusted_proposal_replacing_uncommitted_conflict(proposal.clone())?;",
            "next.insert_trusted_proposal_replacing_uncommitted_conflict(proposal.clone())?;\n"
            "            next.preflight_trusted_proposal_replacement(proposal)?;", 1,
        )
    mutate_rust_item_source_in_context(module, cache_path, item_name, context, old, new)
    original = rebind_reviewed_rust_item_digests(
        module, cache_path, item_name, context,
        ((module._PRODUCTION_LANE_RECOVERY_CACHE_ITEM_SHA256, item_name),),
    )
    try:
        errors = _lane_recovery_cache_owner_contract_errors(module, cache_path)
    finally:
        restore_reviewed_rust_item_digests(original)
    assert any(expected_error in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


def _lane_recovery_cache_owner_contract_errors(
    module, cache_path: Path,
) -> list[str]:
    """Check the actual cache owner independently of the lane adapter source."""

    source = cache_path.read_text(encoding="utf-8")
    errors = []
    items = {}
    for name in (
        "insert_recovered_proposals",
        "preflight_trusted_proposal_replacement",
        "insert_trusted_proposal_replacing_uncommitted_conflict",
    ):
        item = module._require_qualified_rust_item(
            cache_path, source, "LaneBlockSessionCache", name, errors,
            f"lane recovery cache owner {name}",
        )
        items[name] = item
        module._require_rust_item_token_sha256(
            cache_path, item, module._PRODUCTION_LANE_RECOVERY_CACHE_ITEM_SHA256[name],
            name, errors,
        )
    module._require_lane_recovery_cache_source_contracts(cache_path, items, errors)
    return errors


def test_lane_recovery_cache_production_source_is_bound() -> None:
    """The independent cache contract reads and seals its canonical production owner."""

    module = load_checker()
    errors = module._lane_recovery_cache_source_fidelity_errors(ROOT_DIR)
    assert not errors, errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "persist_anchored_sessions",
            "Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {",
            "Ok::<(), String>(()).map_err(|message| {",
            "anchored lane persistence must derive autonomous execution authority",
        ),
        (
            "persist_anchored_sessions",
            "&session.prepare_qc,\n                autonomous_anchor,",
            "&session.prepare_qc,\n                true,",
            "anchored lane persistence must derive autonomous execution authority",
        ),
        (
            "persist_anchored_sessions",
            "if autonomous_certificate && !self.local_can_own_autonomous_payload(&session.proposal)",
            "if !self.local_can_own_autonomous_payload(&session.proposal)",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            "if autonomous_certificate && !self.local_can_own_autonomous_payload(&session.proposal)",
            "if autonomous_certificate && self.local_can_own_autonomous_payload(&session.proposal)",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            ".persist_canonical_autonomous_lane_replica(&candidate)",
            ".persist_committed_lane_block_session(&session, &pops)",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            "if !certified_lane_artifacts_certify_same_decision(",
            "if certified_lane_artifacts_certify_same_decision(",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            ") || replica.bundle.executable_payload().origin_proposal != session.proposal",
            ") && replica.bundle.executable_payload().origin_proposal != session.proposal",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            "replica.bundle.executable_payload().origin_proposal != session.proposal",
            "!replica.bundle.executable_payload().origin_proposal.same_consensus_identity(&session.proposal)",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            "persisted = persisted.saturating_add(1);\n                continue;\n            }\n            // A complete certificate",
            "persisted = persisted.saturating_add(1);\n            }\n            // A complete certificate",
            "public observer persistence must verify its separate exact replica",
        ),
        (
            "persist_anchored_sessions",
            "let pops = self.pops_for_lane_session(&session);",
            "self.persist_autonomous_prepare_availability(&session.proposal, &session.prepare_qc)\n"
            "                .map_err(V2LaneWorkError::Persistence)?;\n"
            "            let pops = self.pops_for_lane_session(&session);",
            "anchored lane persistence must have exactly one committee READY persistence call",
        ),
        (
            "persist_anchored_sessions",
            "move_validation_after_observer",
            "",
            "anchored lane persistence must derive autonomous execution authority",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "if artifact.proposal != *proposal",
            "if !artifact.proposal.same_consensus_identity(proposal)",
            "lane recovery reconstruction must begin from the exact certified Kura artifact",
        ),
        (
            "reconstruct_durable_lane_certificate",
            ".canonical_finalized_autonomous_payload_for_proposal(proposal)",
            ".canonical_autonomous_payload_from_kura(proposal)",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "self.output_guard.close_admission_for_restart();",
            "let _ = &self.output_guard;",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "})?\n                    .is_some();",
            "}).ok().flatten()\n                    .is_some();",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            ".is_some();",
            ".is_none();",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "&& !requester_observes_finalized_public_autonomous_carrier",
            "&& requester_observes_finalized_public_autonomous_carrier",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "artifact.commit_qc.validator_set.contains(sender)",
            "!artifact.commit_qc.validator_set.contains(sender)",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "commit_qc: artifact.commit_qc,",
            "commit_qc: artifact.prepare_qc.clone(),",
            "lane recovery reconstruction must authenticate current or historical membership or exact verified public finality",
        ),
    ),
)
def test_lane_public_certificate_mutations_survive_digest_refresh(
    tmp_path: Path, item_name: str, old: str, new: str, expected_error: str,
) -> None:
    """Resealing an observer branch cannot grant custody or invent finality."""

    module = load_checker()
    relative = Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs")
    lane_path = tmp_path / relative
    lane_path.parent.mkdir(parents=True)
    shutil.copyfile(ROOT_DIR / relative, lane_path)
    baseline_errors = _lane_public_certificate_owner_contract_errors(module, lane_path)
    assert not baseline_errors, baseline_errors
    context = (("impl", "V2LaneWorkAdapter"),)
    if old == "move_validation_after_observer":
        source = lane_path.read_text(encoding="utf-8")
        start = source.index("            Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {")
        validation_end = source.index("            let descriptor = &session.proposal.descriptor;", start)
        observer_end = source.index("            // A complete certificate", validation_end)
        validation = source[start:validation_end]
        observer = source[validation_end:observer_end]
        old, new = validation + observer, observer + validation
    mutate_rust_item_source_in_context(module, lane_path, item_name, context, old, new)
    qualified = f"V2LaneWorkAdapter::{item_name}"
    bindings = (
        ((module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256, qualified),)
        if item_name == "persist_anchored_sessions"
        else ((module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256, item_name),)
    )
    original = rebind_reviewed_rust_item_digests(
        module, lane_path, item_name, context, bindings
    )
    try:
        errors = _lane_public_certificate_owner_contract_errors(
            module, lane_path, item_name
        )
    finally:
        restore_reviewed_rust_item_digests(original)
    assert any(expected_error in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


def _lane_public_certificate_owner_contract_errors(
    module, lane_path: Path, refreshed_item: str | None = None,
) -> list[str]:
    """Check real qualified owners; preserve the separate full integration seal."""

    source = lane_path.read_text(encoding="utf-8")
    errors = []
    lane_items = {}
    lane_ack_items = {}
    for name in ("persist_anchored_sessions", "reconstruct_durable_lane_certificate"):
        item = module._require_qualified_rust_item(
            lane_path, source, "V2LaneWorkAdapter", name, errors,
            f"lane public certificate owner {name}",
        )
        qualified = f"V2LaneWorkAdapter::{name}"
        lane_items[name] = item
        lane_ack_items[qualified] = item
        if name == refreshed_item:
            digest = (
                module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256[qualified]
                if name == "persist_anchored_sessions"
                else module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256[name]
            )
            module._require_rust_item_token_sha256(lane_path, item, digest, name, errors)
    module._require_lane_public_certificate_source_contracts(
        lane_path, lane_ack_items, lane_items, errors
    )
    return errors


def test_merge_execution_validation_cache_semantics_survive_digest_refresh(tmp_path: Path) -> None:
    """Resealing cannot hide weakened merge-execution cache authority."""

    module = load_checker()
    original_seals = dict(module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256)
    mutations = (
        (
            "merge_execution_candidate_validation_memo",
            "state_view_generation,\n            canonical_candidate_bytes,",
            "state_view_generation: state_view_generation.saturating_add(2),\n"
            "            canonical_candidate_bytes,",
            "merge execution cache identity must bind the exact height",
        ),
        (
            "merge_parent_frontier_at_generation",
            "if durable_parent != Some(expected_parent) {",
            "if false {",
            "merge execution cache reuse must bracket an exact durable parent frontier",
        ),
        (
            "validate_merge_candidate_for_active_round",
            "if candidate.execution_batch.is_none() {",
            "if candidate.execution_batch.is_some() {",
            "relay and drain candidates must retain full live production validation",
        ),
        (
            "build_and_memoize_merge_execution_candidate",
            "&& matches!(\n                self.merge_parent_frontier_at_generation(state_view_generation),\n                Ok(MergeCandidateValidation::Ready)\n            )",
            "&& state_view_generation % 2 == 0",
            "only State's validating builder may seed a memo",
        ),
        (
            "mark_global_body_locked",
            "self.validated_merge_execution_candidate = None;",
            "let _ = &self.validated_merge_execution_candidate;",
            "global body lock must invalidate merge execution validation authority",
        ),
        (
            "validate_merge_candidate_for_active_round",
            "#[cfg(test)]\n        self.merge_candidate_validation_checks.set(",
            "self.merge_candidate_validation_checks.set(",
            "merge validation accounting must remain test-only",
        ),
        (
            "refresh_merge_candidates",
            "if persisted_entries.next().is_some() {",
            "if false {",
            "persisted merge reuse must reject competing owners",
        ),
        (
            "refresh_merge_candidates",
            "let PendingMergeStage::Persisted(entry) = &pending.stage",
            "let PendingMergeStage::Certified(entry) = &pending.stage",
            "persisted merge reuse must reject competing owners",
        ),
        (
            "refresh_merge_candidates",
            "| PendingMergeStage::Persisted(_) => None,",
            "=> None,\n                PendingMergeStage::Persisted(entry) => Some(crate::merge::MergeLedgerCandidate::from(entry)),",
            "persisted merge owners must never reenter collecting candidate reconstruction",
        ),
        (
            "refresh_merge_candidates",
            "move_parent_check_after_persisted",
            "",
            "persisted merge reuse must authenticate current authority before its terminal return",
        ),
        *(
            (
                "refresh_merge_candidates",
                comparison,
                "false",
                "persisted merge reuse must reject competing owners and bind the full candidate",
            )
            for comparison in (
                "key.epoch_id != expected_epoch",
                "key.view != active_view",
                "candidate.epoch_id != key.epoch_id",
                "candidate.view != key.view",
                "candidate.carrier_height != self.context.height",
                "candidate.carrier_parent_hash != expected_parent",
                "key.digest != expected_digest",
                "entry.merge_qc.epoch_id != key.epoch_id",
                "entry.merge_qc.view != key.view",
                "entry.merge_qc.message_digest != key.digest",
                "authorized != Some((key.digest, candidate, candidate_bytes))",
            )
        ),
    )
    try:
        for index, (item_name, old, new, expected_error) in enumerate(mutations):
            fixture_root = tmp_path / f"merge-execution-cache-{index}"
            lane_path = (
                fixture_root
                / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
            )
            lane_path.parent.mkdir(parents=True)
            shutil.copy2(
                ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
                lane_path,
            )
            shutil.copy2(
                ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_runner.rs",
                lane_path.with_name("v2_runner.rs"),
            )
            baseline_errors: list[str] = []
            module._require_merge_execution_validation_cache_contract(
                lane_path,
                lane_path.read_text(encoding="utf-8"),
                baseline_errors,
            )
            assert not baseline_errors, baseline_errors
            if old == "move_parent_check_after_persisted":
                source = lane_path.read_text(encoding="utf-8")
                original_item = next(
                    candidate
                    for candidate in module.rust_items(source, item_name)
                    if candidate.brace_context == (("impl", "V2LaneWorkAdapter"),)
                )
                guard_start = original_item.source.index("        if parent_header.hash() != expected_parent")
                guard_end = original_item.source.index("        let local_is_leader =", guard_start)
                parent_guard = original_item.source[guard_start:guard_end]
                mutated_item = original_item.source.replace(parent_guard, "", 1)
                marker = "        let authorized_candidate = signing_guard"
                assert mutated_item.count(marker) == 1
                mutated_item = mutated_item.replace(marker, parent_guard + marker, 1)
                assert source.count(original_item.source) == 1
                lane_path.write_text(source.replace(original_item.source, mutated_item, 1), encoding="utf-8")
            else:
                mutate_rust_item_source_in_context(
                    module,
                    lane_path,
                    item_name,
                    (("impl", "V2LaneWorkAdapter"),),
                    old,
                    new,
                )
            item = next(
                candidate
                for candidate in module.rust_items(
                    lane_path.read_text(encoding="utf-8"), item_name
                )
                if candidate.brace_context
                == (("impl", "V2LaneWorkAdapter"),)
            )
            qualified_name = f"V2LaneWorkAdapter::{item_name}"
            module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256[
                qualified_name
            ] = module._rust_item_token_sha256(item)
            errors: list[str] = []
            module._require_merge_execution_validation_cache_contract(
                lane_path,
                lane_path.read_text(encoding="utf-8"),
                errors,
            )
            assert any(
                expected_error in error
                and "exact reviewed token digest" not in error
                for error in errors
            ), errors
            assert not any("exact reviewed token digest" in error for error in errors), errors
            module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256.clear()
            module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256.update(
                original_seals
            )
    finally:
        module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256.clear()
        module._PRODUCTION_MERGE_EXECUTION_CACHE_ITEM_SHA256.update(original_seals)

@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_core/src/lib.rs",
            "pub enum NetworkMessage",
            "CertifiedMergeSidecar(Arc<CertifiedMergeSidecarMessage>),",
            "CertifiedMergeSidecar(Box<CertifiedMergeSidecarMessage>),",
            "every exact-output network payload class must use an immutable shared carrier",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct OutboundTransfer",
            "chunks: Vec<Arc<CertifiedMergeSidecarMessage>>",
            "chunks: Vec<CertifiedMergeSidecarMessage>",
            "sidecar responses must cache each immutable fixed-boundary payload once for every source cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "let message = Arc::clone(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\"),\n"
            "                        );",
            "let message = Arc::new(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\")\n"
            "                                .as_ref()\n"
            "                                .clone(),\n"
            "                        );",
            "sidecar drainage must clone only the cached Arc",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "let projection = admission.projection();",
            "let _rebuilt_payload = Vec::<u8>::new().to_vec();\n"
            "        let projection = admission.projection();",
            "per-source sidecar drainage and acknowledgement must never reconstruct cached payload bytes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) enum V2LaneWorkEffect",
            "message: Arc<CertifiedMergeSidecarMessage>,",
            "message: CertifiedMergeSidecarMessage,",
            "the lane effect must preserve the exact immutable sidecar carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "pub(crate) fn post_certified_merge_sidecar_with_reply_routes(",
            "let data = NetworkMessage::CertifiedMergeSidecar(message);",
            "let data = NetworkMessage::CertifiedMergeSidecar(Arc::new((*message).clone()));",
            "worker sidecar dispatch must install the existing Arc without reconstruction",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            "Arc::clone(&message),",
            "Arc::new((*message).clone()),",
            "runner sidecar dispatch must preserve the exact peer, complete route set, and immutable message pointer",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE",
            "nonzero!(2_usize)",
            "nonzero!(3_usize)",
            "certified sidecar per-source sessions must remain exactly two",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE",
            "nonzero!(16_usize * 1024 * 1024)",
            "nonzero!(17_usize * 1024 * 1024)",
            "certified sidecar per-source bytes must remain exactly 16 MiB",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE",
            "nonzero!(4_usize)",
            "nonzero!(5_usize)",
            "certified sidecar per-source request gates must remain exactly four",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn from_admitted_reply(",
            "semantic_target: flush_identity.semantic_target().clone(),",
            "semantic_target: chunk.responder.clone(),",
            "sidecar writer-flush admission must bind the opaque source, exact route, actor ticket and clone-shared claim with immutable payload and cursors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "check_production_reliable_flush_worker_transition(flush_trace)\n"
            "                            .ok_or_else",
            "Some(flush_trace)\n"
            "                            .ok_or_else",
            "writer-flush ownership must consume the checked transition token before removing its target-local witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            ".bind_confirmed_worker_trace(flush_trace)",
            ".bind_confirmed_worker_trace(ProductionReliableFlushTraceProjection::default())",
            "a successful writer occurrence must bind its exact confirmed worker trace before lane admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "if let Some(admission) = pending_flush.sidecar_admission.take() {\n"
            "                        self.admitted_sidecar_chunks.push_back(admission);\n"
            "                    }",
            "let _ = pending_flush.sidecar_admission.take();",
            "only a successful peer-writer flush may create a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "let route_state = self\n"
            "                        .fanouts",
            "if let Some(admission) = pending_flush.sidecar_admission.take() {\n"
            "                        self.admitted_sidecar_chunks.push_back(admission);\n"
            "                    }\n"
            "                    let route_state = self\n"
            "                        .fanouts",
            "closed writer ownership must not manufacture a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn project_sidecar_receipt_completions(",
            "admission.matches_materialized_chunk(message) && admission.is_bound_to_source(route)",
            "admission.matches_materialized_chunk(message)",
            "retained sidecar flush completion must match the immutable chunk and exact authenticated source before advancing only that route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "CertifiedMergeSidecarMessage::Request(_)\n"
            "                    | CertifiedMergeSidecarMessage::Close(_)\n"
            "                    | CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "CertifiedMergeSidecarMessage::Request(_) => Some((\n"
            "                        post.clone(),\n"
            "                        reply_route.clone(),\n"
            "                        message_cursor_before,\n"
            "                        message_cursor_after,\n"
            "                    )),\n"
            "                    CertifiedMergeSidecarMessage::Close(_)\n"
            "                    | CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "only an immutable certified response chunk may create a writer-flush receipt from its exact route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "self.sidecar_control_units() >= self.sidecar_admission_capacity",
            "self.sidecar_control_units() > self.sidecar_admission_capacity",
            "sidecar receipt capacity must reject at the exact full boundary",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            ".post_reply_recoverable_with_flush_ack_at_attempt(\n"
            "                        post,\n"
            "                        reply_route,\n"
            "                        ticket,\n"
            "                        reply_writer_timeout_attempt,\n"
            "                    )?",
            ".post_reply_recoverable(post, reply_route, ticket)\n"
            "                .map(|()| None)?",
            "production reply output must retain every exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            "Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack))",
            "{ drop(flush_ack); Ok(ExactOutputAttemptOutcome::Admitted) }",
            "production reply output must retain every exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn drain_certified_merge_sidecar_chunk_admissions(",
            "limit.min(pending.admitted_sidecar_chunks.len())",
            "limit.min(pending.flushing_sidecar_chunks.len())",
            "receipt drainage may consume only successfully flushed sidecar admissions",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_link_transition(worker_trace, occurrence).ok_or(",
            "check_production_reliable_flush_link_transition(worker_trace, worker_trace).ok_or(",
            "lane application must consume checked worker/link tokens for the exact accepted occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.projection_matches_identity(&admission.flush_identity) {",
            "if false {",
            "lane application must consume checked worker/link tokens for the exact accepted occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "|| projection.chunk_cursor_before != chunk_index",
            "|| false",
            "lane application must validate the immutable message and chunk cursors before transport preflight",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "preflight_reliable_flush_outbound(self, admission, &gate, chunk_index, count)?",
            "preflight_reliable_flush_outbound(self, admission, &gate, 0, count)?",
            "the exact gate, source route, shared bytes and cursors must preflight into one immutable application plan before claiming completion",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGateAttempt {",
            "cursor: ServerResponseCursor,",
            "cursor: usize,",
            "sidecar request gates must retain exact materialization authority, retry state, and a source-local pending-or-terminal cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "enum ServerResponseCursor {",
            "Complete,",
            "PendingZero,",
            "sidecar gate history must preserve terminal completion across exact, later-delivery, and reconnected observations",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn prune_server_gates(",
            "let reclaimed = self.reclaim_inactive_outbound_attempts(now)?;",
            "let reclaimed = 0;",
            "sidecar gate pruning must preserve semantic ownership until an authenticated close floor retires it",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn route_update(",
            ".source_update_from(prior)",
            ".source_update_from(candidate)",
            "same-source sidecar route update must use the canonical monotonic update kernel",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if prior.cursor == ServerResponseCursor::Complete {",
            "if false && prior.cursor == ServerResponseCursor::Complete {",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let ServerResponseCursor::Pending(resume_chunk) = attempt.cursor else {\n"
            "                continue;\n"
            "            };",
            "let resume_chunk = match attempt.cursor {\n"
            "                ServerResponseCursor::Pending(chunk) => chunk,\n"
            "                ServerResponseCursor::Complete => 0,\n"
            "            };",
            "completed sidecar sources must never regain materialized output",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "cursor = ServerResponseCursor::Complete;",
            "cursor = ServerResponseCursor::Pending(0);",
            "sidecar drainage must persist terminal completion rather than a replayable chunk-zero cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.flush_identity.claim_writer_flush_once() {",
            "if false {",
            "the clone-shared writer claim must be the sole linearization point before the exact checked application is compared and durably published",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_application_transition(prospective_application)",
            "check_production_reliable_flush_worker_transition(worker_trace)",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "check_production_reliable_flush_link_transition(worker_trace, prospective_application)",
            "check_production_reliable_flush_link_transition(worker_trace, occurrence)",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "let prospective_application = checked_application.into_projection();",
            "let prospective_application = prospective_application;",
            "the clone-shared writer claim must remain behind the checked application/link gates and opaque-token consumption",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.materialization_retryable = false;\n"
            "                    return Ok(ServerRequestAdmission::Existing);",
            "attempt.materialization_retryable = true;\n"
            "                    return Ok(ServerRequestAdmission::Existing);",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "let retry_chunk = attempt.in_flight_chunk.unwrap_or(attempt.next_chunk);",
            "let retry_chunk = 0;",
            "a replacement writer tenure must retry the source's current chunk",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if attempt.in_flight_chunk.is_none() && !attempt.queued {",
            "if !attempt.queued {",
            "a later delivery with an in-flight chunk must refresh only its source route without queueing a concurrent copy",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.cursor = ServerResponseCursor::Pending(retry_chunk);",
            "attempt.cursor = ServerResponseCursor::Pending(0);",
            "an observed source update must never reset a retained sidecar cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "candidate.same_delivery(admitted)",
            "candidate.same_tenure(admitted)",
            "materialization must consume the exact admitted delivery capability",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let mut remaining_global_sessions = self\n"
            "            .outbound_session_capacity\n"
            "            .saturating_sub(self.outbound_attempt_count());",
            "let mut remaining_global_sessions = usize::MAX;",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "self.source_outbound_count(source) >= self.limits.outbound_sessions_per_source",
            "self.source_outbound_count(source) > self.limits.outbound_sessions_per_source",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            ".saturating_add(response_len)\n"
            "                    > self.limits.outbound_bytes_per_source",
            ".saturating_add(response_len)\n                    > usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty()",
            "> self.outbound_byte_capacity",
            "> usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if remaining_global_sessions == 0",
            "capacity_rejected_attempts.push(source.clone());",
            "return Err(MergeSidecarError::Capacity(\"outbound response budget\"));",
            "one saturated sidecar source must not erase independently admissible same-request sources",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if !Self::alternate_source_is_authorized",
            "next_chunk: 0,",
            "next_chunk: prior.resume_chunk,",
            "a newly observed alternate sidecar source must begin at chunk zero",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn drain_outbound_chunks_inner(",
            "attempt.in_flight_chunk = Some(index);",
            "attempt.next_chunk = index.saturating_add(1);",
            "preserve the exact source route, and mark only an in-flight cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "!existing.request.same_occurrence_except_close_floor(request)",
            "false && !existing.request.same_occurrence_except_close_floor(request)",
            "duplicate sidecar admission must preserve canonical occurrence identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn alternate_source_is_authorized(",
            "match candidate {",
            "return true; match candidate {",
            "alternate sidecar sources must retain live route authority without treating recovered peer ownership as a process-local capability",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if &request.requester != sender || &request.responder != local_peer {",
            "if false && (&request.requester != sender || &request.responder != local_peer) {",
            "sidecar request admission must bind authenticated sender, responder, semantic target, and active route",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn cancel_unmaterialized_server_request(",
            "Self::release_authorized_server_request_attempts(gate);",
            "let _ = gate;",
            "failed sidecar materialization must preserve route/cursor history",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if !prior.materialization_retryable {",
            "if false && !prior.materialization_retryable {",
            "an exact failed-materialization retry must preserve only its source-local retryability and re-enter durable fair selection",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn release_unsent_request(",
            "let attempt = assembly\n"
            "            .current\n"
            "            .take()",
            "return; let attempt = assembly\n            .current\n            .take()",
            "an unsent sidecar request must restore the exact holder cursor, close its durable sequence, and persist before retry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "if acknowledged {",
            "if true {",
            "lane work may schedule the next chunk only after the exact receipt and next pending writer identity are durable",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "operation.complete();",
            "drop(operation);",
            "lane sidecar ACK application may complete only after every successor post is retained",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn effect_count(",
            "self.effects\n"
            "            .len()\n"
            "            .saturating_add(self.sidecar_effects.len())",
            "0",
            "lane scan rank must count both ordinary and source-owned sidecar effects",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn requeue_effect(",
            "match effect {",
            "drop(effect); return true; match effect {",
            "lane requeue must return the exact unserviceable occurrence to its bounded owner lane",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_post(",
            "let MergeSidecarPost {\n"
            "            peer,\n"
            "            reply_route,\n"
            "            message,\n"
            "        } = post;",
            "let MergeSidecarPost {\n"
            "            peer: _,\n"
            "            reply_route,\n"
            "            message,\n"
            "        } = post;\n"
            "        let peer = self.local_peer.clone();",
            "lane sidecar post conversion must preserve the exact peer and message while stripping only GenerationHint reply-route ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn prune_finalized_merge_sidecars(",
            ".map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;",
            ".ok();",
            "finalized sidecar pruning must remain fail-stop and Kura-bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn is_pending(",
            "fanout.has_dispatchable_target()",
            "false",
            "pending exact output must include dispatchable fanouts, writer flushes, and undrained receipts without spinning on parked ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "self.admitted_sidecar_chunks.clear();",
            "let _ = &self.admitted_sidecar_chunks;",
            "applied-height handoff must retire every volatile sidecar completion state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_io_execution.rs",
            "fn retain_returned(",
            "if post.data.exact_output_hash() != *expected_hash {",
            "if false && post.data.exact_output_hash() != *expected_hash {",
            "returned actor post must retain the exact pinned payload identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            ".any(|(message, expected_hash)| message.exact_output_hash() != *expected_hash)",
            ".any(|(message, expected_hash)| false && message.exact_output_hash() != *expected_hash)",
            "applied-height handoff must preflight every pinned payload before classification",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "blocked_sources.insert(attempted_source);",
            "let _ = attempted_source;",
            "exact-output drive_with_budget_ack declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn apply_reply_route_update(",
            "self.current = None;",
            "self.message_index = 0; self.current = None;",
            "a same-source reconnect must not reset its retained exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesce_reservation_additions_for_plan(",
            "ReplyTargetMerge::Update { .. } => 0,",
            "ReplyTargetMerge::Update { .. } => full_mask,",
            "ordinary same-source updates retain reservation ownership while only a new source charges the candidate cursor suffix",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn preview_coalesce_plan(",
            "target.2 = false;",
            "target.1 = 0; target.2 = false;",
            "the coalesce preview must preserve the retained message cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_sources_excluding(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain every outstanding source/FIFO class",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_reservation_counts(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain their reservation ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan(&self, candidate: &Self)",
            "self.reply_target_merge_plan_with_hooks(candidate, |_| {}, || {})",
            "self.reply_target_merge_plan_after_candidate_prune(candidate, |_| {})",
            "the no-hook production coalescing wrapper must delegate to the receipt-bound route-history kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".project_retained_reply_routes(prune_receipt)",
            ".project_retained_reply_routes(prune_receipt.clone())",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_with_receipt(&candidate_routes)",
            ".merge(&candidate_routes)",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".any(|route| route.same_delivery(candidate_route))",
            ".any(|route| route.same_source(candidate_route))",
            "the authoritative merged route snapshot must reuse the immutable joint tenure/delivery freshness kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "                    update,\n                });",
            "                    update: NetworkReplyRouteSourceUpdate::Exact,\n"
            "                });",
            "same-source coalescing must preserve the retained source cursor while updating only its authenticated route capability",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "message_index: candidate_target.message_index,\n"
            "                        reply_writer_timeout_attempt: candidate_target.reply_writer_timeout_attempt,\n"
            "                        current: None,",
            "message_index: 0,\n"
            "                        reply_writer_timeout_attempt: candidate_target.reply_writer_timeout_attempt,\n"
            "                        current: None,",
            "an appended source must preserve its candidate cursor and parked state while starting without actor-post, admission-ticket, or writer-flush ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capacity_available_for(",
            "pending.coalesce_reservation_additions_for_plan(fanout, &plan.targets)?",
            "fanout.admission_reservation_counts()?",
            "capacity preflight must enforce route-source geometry before charging only newly appended source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesced_target_geometry_available(",
            "&& target_count <= plan.reply_routes.source_capacity()",
            "&& true",
            "coalesced reply attempts must fit both the configured fanout bound and the actor-derived source-capacity geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".source_update_from_snapshot(prior_route)",
            ".source_update_from(prior_route)",
            "the authoritative merged route snapshot must reuse the immutable joint tenure/delivery freshness kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn classified_with_reply_routes(",
            "Self::classified_with_route_history(messages, peers, routes, Some(reply_routes))",
            "Self::classified_with_route_history(messages, peers, routes, None)",
            "reply fanout construction must preserve the complete bounded route history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_io_execution.rs",
            "fn classified_with_route_history(",
            "            reply_routes,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "            reply_routes: None,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "fanout construction must store the complete authoritative live-and-tombstone reply history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "let retained_routes = self.reply_routes.clone().ok_or_else",
            "let retained_routes = candidate.reply_routes.clone().ok_or_else",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "plan.push(ReplyTargetMerge::Update {",
            "plan.push(ReplyTargetMerge::Reactivate {",
            "no coalescing path may reset a retained terminal reply cursor from a newly materialized candidate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            ".commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);",
            ".coalesce_retry(&fanout)?;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            "self.source_fifo_owners = next_source_fifo_owners;",
            "let _ = next_source_fifo_owners;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "self.reply_routes = Some(plan.reply_routes.clone());",
            "self.reply_routes = None;",
            "atomic coalesce commit must install the complete route and fair-ingress histories",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn exact_target_geometry(",
            "Some(reply_routes.clone()),",
            "None,",
            "lane preflight expands every authenticated source into an independent exact target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn advance_after_attempt(",
            "self.unregister_source_fifo_owner(fifo_id, source)?;",
            "let _ = (fifo_id, source);",
            "admission advances only the completed class/source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "fn start_inner(",
            ".map(|entry| entry.validator.clone())",
            ".filter(|_| false).map(|entry| entry.validator.clone())",
            "production bounds protocol fanout by roster and source geometry while charging the shared pool only for the independently reserved authenticated reply sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "fn enqueue_owned_exact_reply_routes_while_guarded(",
            "PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(",
            "PendingExactFanout::claimed_with_routes(",
            "exact replies expand all authenticated sources without changing semantic identity and preserve bounded route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "rollover_claim.validate_fanout(messages, peers)?;",
            "let _ = (messages, peers);",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "if !scope.covers(artifact) {",
            "if false && !scope.covers(artifact) {",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_exact_fanout_while_guarded(",
            "PendingExactFanout::claimed(messages, peers, rollover_claim)?",
            "PendingExactFanout::claimed(messages, peers, ExactOutputRolloverClaim::Exact)?",
            "every production exact fanout must enter the corridor with its typed claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "durable_history.ok_or_else(|| {\n                \"Sumeragi v2 durable response lacks an independently readable history source\"",
            "Some(durable_history.unwrap()).ok_or_else(|| {\n                \"Sumeragi v2 durable response lacks an independently readable history source\"",
            "applied-height handoff must independently reread every durable Kura response source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn durable_history_source_covers(",
            "|| response.certificate != source.commit_qc",
            "|| false",
            "durable CommitQC response must match its exact Kura finality source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn durable_history_source_covers(",
            "|| !proof.covers_message_in_network(source_network_id, message)",
            "|| false",
            "durable body response must match its exact canonical Kura block",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn durable_history_source_covers(",
            "|| certificate.commit_qc != source.commit_qc",
            "|| false",
            "durable lane certificate must match its exact certified Kura source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "fn post_durable_history_response_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "global historical response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn post_durable_lane_certificate_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "historical lane response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(",
            "Some(self.kura.as_ref()),",
            "None,",
            "production handoff must pass exact lane and Kura authorities into retirement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "round.context_id == context_id && round.height == height",
            "round.context_id == context_id",
            "durable rollover classification must bind the exact artifact context and height",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "ProgressReconstruction::Retransmit",
            "ProgressReconstruction::Exact",
            "exact-output applied_height_reconstruction_covers declaration",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn validate_applied_height_output_handoff_authority(",
            "|| receipt.artifact_hash() != HashOf::new(artifact)",
            "|| false",
            "applied-height handoff requires the exact Kura receipt and finality artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "false && self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "lane rollover authority must bind the exact finality artifact and consult its proposal-keyed durable source before height supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.durable_sessions.get(&proposal_hash)",
            "self.durable_sessions.values().next()",
            "lane rollover authority must bind the exact finality artifact and consult its proposal-keyed durable source before height supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "validate_superseded_lane_output(message)?;",
            "let _ = message;",
            "non-winning lane output must be validated before artifact-bound supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn persistent(",
            "application_receipt_hash.as_ref(),",
            "durable_artifact_hash.as_ref(),",
            "lane durable source must commit finality, certificate, and application receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "block.header().height().get() != finality_artifact.height\n"
            "            || block.hash() != finality_artifact.block_hash",
            "false",
            "lane authority builder must derive its bounded ordinary and autonomous "
            "winner set from the exact canonical block",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "|| receipt.application_block_hash != finality_artifact.block_hash",
            "|| false",
            "lane authority builder must bind every winner to the exact applied artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn reconstruct_durable_lane_certificate(",
            "self.kura.read_certified_lane_block_artifact(",
            "self.kura.read_certified_lane_block_artifact_unchecked(",
            "lane recovery reconstruction must begin from the exact certified Kura artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn serve_durable_lane_certificate(",
            "reply_routes: Some(reply_routes),",
            "reply_routes: None,",
            "lane recovery emitter retains every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "let mut merged = retained.clone();",
            "let mut merged = candidate.clone();",
            "lane effect coalescence atomically commits canonical history maintenance and reports success only for a retained live candidate delivery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "merged.merge_observed_with_receipt(candidate)",
            "merged.merge(candidate)",
            "lane effect coalescence atomically commits canonical history maintenance and reports success only for a retained live candidate delivery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_lane_work_effect_reply_routes_after_route_merge<AfterRouteMerge>(",
            "if !lane_work_effect_reply_routes_have_valid_shape(candidate) {",
            "if !lane_work_effect_reply_routes_are_valid(candidate) {",
            "inactive duplicates still reach maintenance",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn lane_work_effect_key(",
            "encoded.push(4);",
            "encoded.push(0);",
            "durable lane response effect identity must include its distinct tag, peer, and certificate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
            "fn consume_prepared_dequeued_v2_ingress(",
            "services.post_durable_history_response_on_reply_routes_with_permit(",
            "services.post_durable_history_response_with_permit(",
            "historical global responses preserve the complete prevalidated route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            ".post_durable_lane_certificate_on_reply_routes(\n"
            "                    peer,\n"
            "                    reply_routes,\n"
            "                    ingress_ownership,\n"
            "                    certificate,\n"
            "                )",
            ".post_lane_block(peer, BlockMessage::LaneBlockCertificate(Box::new(certificate)))",
            "historical lane dispatch preserves every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects_with_progress(",
            "let scan_limit = lane_work.effect_count();",
            "let scan_limit = limit.max(1);",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects_with_progress(",
            "continue;",
            "break;",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects_with_progress(",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts before selecting owned work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "match dispatch_lane_work_effect(services, next_effect)? {",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts after complete and source-retained exact handoffs",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "pub(super) fn preflight_finalized_lane_rollover(",
            "let _ = service_historical_recovery_tick(lane_work, services)?;",
            "let _ = &lane_work;",
            "finalized-lane preflight must keep the active predecessor alive",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "let _ = retry_exact_output_and_apply_sidecar_admissions(\n"
            "        &mut lane_work,\n"
            "        services,\n"
            "        control_queue_capacity,\n"
            "    )?;",
            "let _ = services.retry_pending_exact_output();",
            "durable finalization must retry exact output and reject rollover until every predecessor-height recovery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn persist_anchored_sessions(",
            "self.hydrate_canonical_lane_artifacts()?;",
            "let _ = &self.lane_sessions;",
            "late canonical lane hydration must precede committed-session collection",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "adapter.ensure_globally_applied_lane_receipts_durable()?;\n"
            "        construction.complete();",
            "adapter.ensure_globally_applied_lane_receipts_durable()?;\n"
            "        adapter.hydrate_canonical_lane_artifacts()?;\n"
            "        construction.complete();",
            "production constructor must remain carrier-silent before exact Queue installation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn activate_after_lane_drain_queue_install(",
            "self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;",
            "let _ = installed_queue;",
            "one-shot startup activation must authenticate the installed Queue",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn claim_runner_lifecycle_process_generation(",
            "NodeRole::Validator => kura\n"
            "            .claim_autonomous_lifecycle_process_generation(",
            "NodeRole::Validator => kura\n"
            "            .read_autonomous_lifecycle_process_generation(",
            "the configured-role process generation helper must durably claim validator ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn claim_runner_lifecycle_process_generation(",
            "match role {",
            "if local_validator_index(context, local_peer, role)?.is_none() {\n"
            "        return Ok(None);\n"
            "    }\n"
            "    match role {",
            "the configured-role process generation helper must not consult height-local roster membership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "pub(super) fn run_non_pending_lifecycle_loop(",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?;",
            "let _ = &queue;",
            "runner startup must install the exact Queue before the one-shot carrier activation seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn hydrate_canonical_lane_artifacts(",
            "        self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "        raw_proposals.reverse();\n"
            "        self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "raw lane hydration must install independent chains in canonical deterministic order as one complete bounded recovery batch before publishing payloads or historical READY",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "lane_work.persist_anchored_sessions()?;",
            "let _ = &lane_work;",
            "finalized output rollover must durably settle every predecessor owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            ".durable_lane_rollover_authority(artifact)?",
            "None",
            "finalized output rollover must durably settle every predecessor owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
            "fn rollover_finalized_height_outputs(",
            "lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority)?;",
            "lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority.clone())?;",
            "finalized output rollover must durably settle every predecessor owner",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "reply_source_capacity,\n"
            "            server_roster_digest,\n"
            "            server_stream_capacity,\n"
            "            outbound_session_capacity,",
            "reply_source_capacity,\n"
            "            server_roster_digest,\n"
            "            server_stream_capacity,\n"
            "            outbound_session_capacity: 0,",
            "sidecar source geometry must reject zero and install every checked corridor bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            "tick_close_next: false,",
            "tick_close_next: true,",
            "sidecar request/close fairness must service an initial progress-bearing request before alternating closes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn tick_bounded(",
            "if self.timeout_retry_close_deferred {",
            "if false {",
            "a newly timed-out sidecar fetch may preempt administrative closure only once before retained Close debt wins the bounded service slot",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn begin_request_or_close(",
            "self.timeout_retry_close_deferred = false;",
            "let _ = &self.timeout_retry_close_deferred;",
            "servicing a sidecar Close must discharge retained timeout-preemption debt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn release_authorized_server_request_attempts(",
            "matches!(attempt.cursor, ServerResponseCursor::Pending(_));",
            "false;",
            "transiently rejected response work must release materialization authority",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn park_authorized_server_request_attempts(",
            "for attempt in gate\n            .attempts",
            "return; for attempt in gate\n            .attempts",
            "parking a materialized response must consume retryability while retaining each source route and resume cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty() && capacity_rejected_attempts.is_empty()",
            "Self::park_authorized_server_request_attempts(gate, now);",
            "let _ = (gate, now);",
            "completed-race and admitted materialized response work must pass through terminal parking",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "attempt.materialization_retryable =\n"
            "                matches!(attempt.cursor, ServerResponseCursor::Pending(_));",
            "attempt.materialization_retryable = false;",
            "partial materialization must keep capacity-partitioned pending sources retryable after shared bytes retire",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn reclaim_inactive_outbound_attempts(",
            "gate_attempt.cursor = ServerResponseCursor::Pending(resume_chunk);",
            "let _ = resume_chunk;",
            "inactive sidecar parking must remain pending at the exact unacknowledged source cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn reclaim_inactive_outbound_attempts(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "inactive sidecar reclamation must publish every projected durable cursor before removing ephemeral writers or shared bytes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_limits_and_server_stream_capacity(",
            ".checked_mul(limits.outbound_sessions_per_source)",
            ".saturating_mul(limits.outbound_sessions_per_source)",
            "sidecar global capacity must be checked from the configured authenticated-source geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn derive_server_request_capacities(",
            ".checked_mul(reply_source_capacity)",
            ".saturating_mul(reply_source_capacity)",
            "sidecar responder gates and per-source attempts must enforce the protocol roster bound and use checked products of the authenticated-source geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn next_server_request_materialization(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "fair sidecar materialization must persist the selected requester cursor before granting any live terminating lookup authority",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "next_chunk: resume_chunk,",
            "next_chunk: 0,",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn apply_reliable_flush_application(",
            "attempt.queued = true;\n            transport\n"
            "                .outbound_order\n"
            "                .push_back((plan.gate.key.clone(), plan.gate.source.clone()));",
            "let _ = &attempt.queued;",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_outbound_chunks_durable(",
            "self.persist_lifecycle_state()?;",
            "let _ = &lifecycle_changed;",
            "production sidecar drainage must durably publish every changed pending identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "#[cfg(test)]\n    fn drain_outbound_chunks(",
            "#[cfg(test)]",
            "#[cfg(any())]",
            "raw non-durable sidecar drainage must remain exactly #[cfg(test)]",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "const MAX_CERTIFIED_MERGE_SEMANTIC_PEERS: usize",
            "MAX_VALIDATORS_PER_HEIGHT;",
            "1;",
            "certified merge semantic request history must have exactly one top-level protocol-roster bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct MergeSidecarRuntimeGeometryV3",
            "semantic_peer_capacity: u64,",
            "semantic_peer_capacity: u32,",
            "durable sidecar geometry must advertise its validator-scoped semantic-peer bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn lifecycle_runtime_geometry_v3(",
            "semantic_peer_capacity: as_u64(MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)?,",
            "semantic_peer_capacity: as_u64(self.reply_source_capacity)?,",
            "lifecycle geometry must fingerprint the validator-scoped semantic-peer bound independently of concurrent reply sources",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn advance_piggybacked_close_floor(",
            "!retained_request.same_occurrence_except_close_floor(request)",
            "retained_request.same_occurrence_except_close_floor(request)",
            "accept only the same immutable occurrence and reject regression",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn advance_piggybacked_close_floor(",
            "self.persist_lifecycle_projection(projected)?;",
            "drop(projected);",
            "publish the sole V2 projection before updating live cancellation, gate, or transfer state without rematerializing",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "snapshot.request_streams.len() > MAX_CERTIFIED_MERGE_SEMANTIC_PEERS",
            "snapshot.request_streams.len() > self.reply_source_capacity",
            "lifecycle restoration must bound both semantic stream maps independently from concurrent authenticated-source gates",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn allocate_request_sequence(",
            "&& reclaim.is_none()",
            "&& reclaim.is_some()",
            "requester-side holder rotation must reclaim only quiescent roster-bounded streams",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn ensure_server_stream_slot(",
            "self.server_streams.len() < self.server_stream_capacity",
            "self.server_streams.len() < self.server_request_gate_capacity",
            "stream-slot helper must reject immutable-capacity exhaustion without independently rolling the generation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn server_generation_is_terminal(",
            "&& self.server_request_gates.is_empty()",
            "|| self.server_request_gates.is_empty()",
            "every stream terminal and every gate, transfer, flush-order, and pending-closure owner empty",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn server_generation_is_terminal(",
            "&& self.pending_server_closures.is_empty()",
            "|| self.pending_server_closures.is_empty()",
            "every stream terminal and every gate, transfer, flush-order, and pending-closure owner empty",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn transition_server_service_generation(",
            "if !self.server_generation_is_terminal() {",
            "if false && !self.server_generation_is_terminal() {",
            (
                "ordinary responder generation rollover must occur only for a full terminal table",
                "ordinary responder generation transition must prepare without mutation, reject nonterminal state, and only then commit the prepared fence",
            ),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "self.ensure_server_stream_slot(sender)?;",
            "let _ = sender;",
            "full current-generation responder table rejects before pruning or mutating lifecycle state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "if !self.server_streams.contains_key(sender) {\n"
            "            return Ok(close_ack());\n"
            "        }",
            "if !self.server_streams.contains_key(sender) {\n"
            "            let _ = close_ack();\n"
            "        }",
            "unknown current-generation Close must be acknowledged without allocating responder stream geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn new(payload: MergeSidecarLifecyclePayloadV3)",
            "let payload_hash = HashOf::new(&payload);",
            "let payload_hash = HashOf::new(&());",
            "merge-sidecar crash-safe lifecycle production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn integrity_is_valid(&self)",
            "self.payload_hash == HashOf::new(&self.payload)",
            "self.payload_hash != HashOf::new(&self.payload)",
            "the durable sidecar snapshot integrity check must bind the complete canonical payload",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "if !snapshot.integrity_is_valid() {",
            "if false && !snapshot.integrity_is_valid() {",
            "lifecycle recovery must accept only canonical integrity-bound V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "decode_from_bytes::<MergeSidecarLifecycleSnapshotV3>",
            "decode_from_bytes::<UnsupportedMergeSidecarLifecycleSnapshotV1>",
            "production lifecycle recovery must never decode the legacy V1 negative-test fixture",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn legacy_lifecycle_v1_snapshot_is_rejected_without_migration(",
            'error.contains("migration is not supported")',
            'error.contains("payload digest mismatch") '
            '/* error.contains("migration is not supported") */',
            "the retired-layout regression must require an explicit no-migration recovery failure",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn source_gate_count(",
            "retained.shares_budget_with(source)",
            "!retained.shares_budget_with(source)",
            "per-source sidecar gate accounting must share one stable authenticated-peer budget across every semantic origin",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn source_gate_count_after_close(",
            "&key.0 != sender",
            "&key.0 == sender",
            "close-aware per-source gate accounting must retain every gate from another semantic origin",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "BTreeMap::<ServerRequestBudgetSource, usize>::new()",
            "BTreeMap::<(PeerId, ServerRequestBudgetSource), usize>::new()",
            "durable recovery must aggregate gate ownership by stable authenticated source rather than by semantic requester/source pairs",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "self.source_gate_count(&source)",
            "self.server_request_gates.len()",
            "alternate-source sidecar admission must retain route-set, global-gate, and per-source-gate bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses(",
            "for index in 0..MAX_SERVER_REQUEST_GATES_PER_SOURCE {",
            "for index in 0..1 {",
            "the exact source-cap regression must fill all four gates through one authenticated hub while varying semantic origins",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "if !snapshot.integrity_is_valid() {",
            "if false && !snapshot.integrity_is_valid() {",
            "lifecycle restoration must independently reject a stale typed payload digest before interpreting any semantic floor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn persist_next(",
            "snapshot.payload_hash = HashOf::new(&snapshot.payload);",
            "snapshot.payload_hash = HashOf::new(&());",
            "V3 lifecycle publication must recompute the typed payload digest before each state-slot publication",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn persist_next(",
            "Self::sync_directory(&self.directory)?;",
            "let _ = &self.directory;",
            "V3 lifecycle publication must sync the replaced state slot before publishing its root",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub enum CertifiedMergeSidecarMessage",
            "GenerationHint(CertifiedMergeSidecarGenerationHintV1),",
            "GenerationHint(CertifiedMergeSidecarCloseAckV1),",
            "the certified sidecar wire enum must expose request, close, close acknowledgement, generation fence, and chunk as distinct exhaustive variants",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) enum ServerRequestAdmission",
            "GenerationHint(MergeSidecarPost),",
            "GenerationHint,",
            "server request admission must explicitly distinguish materialization, existing ownership, and a stateless generation hint",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn canonical_hint_id(&self)",
            "self.observed_message_hash.as_ref(),",
            "&version,",
            "the generation hint identity must bind both generations, the exact observed message, and both authenticated peers",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1(pub NonZeroU64);",
            "pub struct CertifiedMergeSidecarSemanticSequenceV1(pub u64);",
            "every exact semantic occurrence coordinate must use a nonzero typed wire value",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarRequestV1",
            "pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "pub semantic_sequence: u64,",
            "Request wire occurrence must carry typed nonzero generation, epoch, and semantic sequence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub struct CertifiedMergeSidecarChunkV1",
            "pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "pub semantic_sequence: u64,",
            "Chunk wire occurrence must copy the typed nonzero generation, epoch, and semantic sequence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerPendingChunkIdentity",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "process-local pending flush identity must retain the typed nonzero request occurrence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerPendingChunkLifecycleV3",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "the durable pending marker must bind the complete generation-scoped request, response, payload, and chunk identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGate",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "live responder gate must retain the full canonical request and every generation-scoped occurrence coordinate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGateLifecycleV3",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "each durable responder gate must retain the full canonical request and every generation, epoch, sequence, source, and pending-marker coordinate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "struct CertifiedSidecarTransferIdentity",
            "semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,",
            "semantic_sequence: u64,",
            "worker exact-transfer identity must retain the typed nonzero sidecar occurrence",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct MergeSidecarLifecyclePayloadV3",
            "server_service_generation: CertifiedMergeSidecarServiceGenerationV1,",
            "server_service_generation: u64,",
            "the sole V3 durable sidecar snapshot must bind canonical runtime, root generation, and roster geometry",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "for legacy in LEGACY_LIFECYCLE_JOURNAL_DIRS {",
            "for legacy in [] {",
            "lifecycle startup must fail closed on every legacy directory before opening or creating sole V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "journal.publish_bootstrap_marker()?;",
            "let _ = &journal;",
            "lifecycle startup must durably publish the generation-zero root before creating the V3 state directory",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn open(",
            "let marker = journal.decode_root_high_water(&journal.root_high_water_path())?;",
            "let marker = MergeSidecarLifecycleRootHighWaterV3::bootstrap();",
            "existing V3 lifecycle state must be selected by the exact durable root high-water",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn decode_snapshot(",
            "snapshot.payload.version != LIFECYCLE_JOURNAL_VERSION_V3",
            "snapshot.payload.version == LIFECYCLE_JOURNAL_VERSION_V3",
            "lifecycle recovery must accept only canonical integrity-bound V3 state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "gate.request.request_id != gate.request.canonical_request_id()",
            "false",
            "responder recovery must recompute the full canonical request and bind its generation, epoch, and semantic sequence to the durable gate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "gate.attempts.len() > source_capacity.unwrap_or(1)",
            "false",
            "responder recovery must reject a gate whose durable attempts exceed its authenticated route-set capacity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "if source_capacity.is_none() && peer == gate.requester",
            "if source_capacity.is_some() && peer == gate.requester",
            "responder recovery must reject synthetic/authenticated source-kind drift and synthetic requester impersonation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn restore_lifecycle_snapshot(",
            "usize::try_from(pending.chunk_count).ok() != Some(expected_chunk_count)",
            "usize::try_from(pending.chunk_count).ok() != Some(index)",
            "responder recovery must reject any pending marker whose generation-scoped request metadata or exact chunk geometry differs from its gate",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_generation_hint(",
            "self.persist_lifecycle_projection(snapshot)?;",
            "drop(snapshot);",
            "requester generation replacement must persist the new generation and unique epoch before retiring any process-local old-generation attempt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn commit_server_service_generation_transition(",
            "self.persist_lifecycle_projection(snapshot)?;",
            "drop(snapshot);",
            "publish the incremented generation and empty responder tables in the root-anchored V3 snapshot before mutating memory",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "request.service_generation > self.server_service_generation",
            "request.service_generation < self.server_service_generation",
            "a canonical future-generation request must reject atomically, while stale input or terminal full-table compaction returns an exact route-free hint before ordinary request-state mutation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "request.service_generation < self.server_service_generation",
            "request.service_generation > self.server_service_generation",
            "a canonical future-generation request must reject atomically, while stale input or terminal full-table compaction returns an exact route-free hint before ordinary request-state mutation",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "close.service_generation > self.server_service_generation",
            "close.service_generation < self.server_service_generation",
            "a canonical future-generation Close must be rejected while a stale-generation Close returns a stateless exact hint before allocating or mutating a responder stream",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_close(",
            "close.service_generation < self.server_service_generation",
            "close.service_generation > self.server_service_generation",
            "a canonical future-generation Close must be rejected while a stale-generation Close returns a stateless exact hint before allocating or mutating a responder stream",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn sidecar_effect_slots(",
            ".filter(|effect| retryable_sidecar_server_control_peer(effect).is_none())",
            ".filter(|effect| retryable_sidecar_server_control_peer(effect).is_some())",
            "reproducible responder controls must not consume progress reservations while all physical sidecar effects remain relay bounded",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn next_sidecar_effect_selection(",
            ".position(|effect| retryable_sidecar_server_control_peer(effect).is_none())",
            ".position(|effect| retryable_sidecar_server_control_peer(effect).is_some())",
            "sidecar scheduling must prioritize progress while granting retryable responder control a bounded weighted turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 3;",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 3;",
            "const SIDECAR_PROGRESS_DRAIN_WEIGHT: u8 = 0;",
            "the sidecar scheduler must give retryable responder control one bounded turn after exactly three progress-bearing drains",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "MergeSidecarTransport::open_durable_with_server_stream_capacity(",
            "MergeSidecarTransport::open_durable(",
            "lane construction must derive the canonical responder roster and restore or open only its exact durable source and stream geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn rehydrate_for_successor(",
            "self.successor_context_id != successor.id()",
            "self.successor_context_id == successor.id()",
            "retained sidecar ownership must bind the exact successor context and consume its durable output handoff before roster-aware rehydration",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "sidecar_progress_drain_credit: SIDECAR_PROGRESS_DRAIN_WEIGHT,",
            "sidecar_progress_drain_credit: 0,",
            "lane construction must initialize the bounded sidecar progress/control drain credit",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn next_sidecar_effect_selection(",
            "if self.sidecar_progress_drain_credit == 0 {",
            "if self.sidecar_progress_drain_credit > 0 {",
            "sidecar scheduling must prioritize progress while granting retryable responder control a bounded weighted turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn retryable_sidecar_server_control_peer(",
            "CertifiedMergeSidecarMessage::GenerationHint(_) if reply_routes.is_some()",
            "CertifiedMergeSidecarMessage::GenerationHint(_) if reply_routes.is_none()",
            "lane retryable responder controls must retain exact reply ownership for CloseAck and GenerationHint during per-peer coalescing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn retryable_sidecar_server_control_peer(",
            "CertifiedMergeSidecarMessage::CloseAck(_) if reply_routes.is_some()",
            "CertifiedMergeSidecarMessage::CloseAck(_) if reply_routes.is_none()",
            "lane retryable responder controls must retain exact reply ownership for CloseAck and GenerationHint during per-peer coalescing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_post_or_restart(",
            "if retryable_server_control {",
            "if false && retryable_server_control {",
            "reserved sidecar handoff may nonfatally drop only reproducible responder control or an inactive response, and must roll back an unsent request before fail-stop",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            ".any(|queued| retryable_sidecar_server_control_peer(queued) == Some(peer))",
            ".any(|queued| retryable_sidecar_server_control_peer(queued).is_some())",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "if self.sidecar_effects.len() >= self.limits.relay_capacity.get() {",
            "if self.sidecar_effects.len() > self.limits.relay_capacity.get() {",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "if self.sidecar_effect_slots() == 0 {",
            "if false && self.sidecar_effect_slots() == 0 {",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            ".rposition(|queued| retryable_sidecar_server_control_peer(queued).is_some())",
            ".rposition(|queued| retryable_sidecar_server_control_peer(queued).is_none())",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_effect(",
            "self.sidecar_effect_keys\n"
            "                    .remove(&lane_work_effect_key(&evicted));",
            "let _ = evicted;",
            "sidecar effect admission must preserve full identity and routes, coalesce retryable controls per peer inside the physical relay bound, and evict a retryable control before rejecting progress",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn next_effect(",
            "self.next_sidecar_effect_selection()\n"
            "                .and_then(|(index, _)| self.sidecar_effects.get(index))\n"
            "                .cloned()",
            "self.sidecar_effects.front().cloned()",
            "lane effect peek must clone the exact weighted progress/control sidecar selection without consuming its credit",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "let (index, retryable_control) = self\n"
            "                    .next_sidecar_effect_selection()\n"
            "                    .expect(\"sidecar effect selected only when present\");",
            "let (index, retryable_control) = (0, false);",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "self.sidecar_progress_drain_credit = SIDECAR_PROGRESS_DRAIN_WEIGHT;",
            "self.sidecar_progress_drain_credit = 0;",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_effects(",
            "self.sidecar_progress_drain_credit.saturating_sub(1);",
            "self.sidecar_progress_drain_credit.saturating_add(1);",
            "lane effect drain must transfer the same weighted selection as peek, retire its key, and update progress/control credit only after ownership transfer",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_request(",
            "return Ok(if self.push_merge_sidecar_post(post) {",
            "self.push_merge_sidecar_post_or_restart(post)?;\n"
            "            return Ok(if true {",
            "lane ingress must treat a stale-generation hint as bounded retryable output while persisting every materialize/existing gate before fair service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_close(",
            "let _ = self.service_next_certified_merge_sidecar_materialization(now)?;",
            "let _ = now;",
            "an authenticated Close must expose its durable prefix, give fair pending materialization one turn, and preserve both progress and bounded retryable control outcomes",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn from_admitted_reply(",
            "reply_writer_timeout_attempt: flush_identity.reply_writer_timeout_attempt(),",
            "reply_writer_timeout_attempt: 0,",
            "sidecar writer-flush admission must bind the opaque source, exact route, actor ticket and clone-shared claim with immutable payload and cursors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "match attempt(post, ticket, &route, reply_writer_timeout_attempt) {",
            "match attempt(post, ticket, &route, 0) {",
            "worker dispatch must pass the target-local adaptive timeout attempt into actor admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "|| flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                            != reply_writer_timeout_attempt",
            "|| flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                            != 0",
            "ordinary reply cursor must remain unchanged while retaining its exact admission and writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "if flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                        != reply_writer_timeout_attempt",
            "if flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                        != 0",
            "sidecar cursor may advance only after retaining its exact admission and writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "pending_flush.reply_writer_timeout_attempt != current_timeout_attempt",
            "pending_flush.reply_writer_timeout_attempt != 0",
            "terminal reply-flush polling must bind the mutable target, retained writer occurrence, and actor acknowledgement to one adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_reply_flushes(",
            "!= pending_flush.reply_writer_timeout_attempt",
            "!= current_timeout_attempt",
            "terminal reply-flush polling must bind the mutable target, retained writer occurrence, and actor acknowledgement to one adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "!= target.reply_writer_timeout_attempt",
            "!= 0",
            "finality handoff must revalidate target, retained writer occurrence, and actor acknowledgement against the same adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "!= pending_flush.reply_writer_timeout_attempt",
            "!= target.reply_writer_timeout_attempt",
            "finality handoff must revalidate target, retained writer occurrence, and actor acknowledgement against the same adaptive timeout attempt",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn covers(&self, other: &Self)",
            "self.requester == other.requester",
            "self.requester != other.requester",
            "sidecar close-prefix dominance must bind the requester",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_close(",
            "return Ok(false);",
            "return Err(MergeSidecarError::UnsolicitedResponse);",
            "a canonical duplicate CloseAck may be a bounded no-op",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_close(",
            "if ack.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {",
            "if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1 {",
            "merge-sidecar crash-safe lifecycle production seam MergeSidecarTransport::acknowledge_close declaration and complete control flow must match the exact reviewed token digest",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_closed_server_prefixes(",
            "std::mem::take(&mut self.pending_server_closures)",
            "self.pending_server_closures.clone()",
            "merge transport must move every coalesced authenticated close prefix exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_request(",
            "let _ = self.apply_closed_server_prefixes();",
            "let _ = false;",
            "lane ingress must treat a stale-generation hint as bounded retryable output while persisting every materialize/existing gate before fair service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar_close(",
            "let close_progress = self.apply_closed_server_prefixes();",
            "let close_progress = false;",
            "an authenticated Close must expose its durable prefix",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn drain_closed_sidecar_prefixes(",
            "std::mem::take(&mut self.closed_sidecar_prefixes)",
            "self.closed_sidecar_prefixes.clone()",
            "lane work must move each dominant close prefix exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn close_certified_sidecar_prefix(",
            "&transfer.requester,",
            "&prefix.requester,",
            "worker close-prefix projection must bind the exact requester",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn close_certified_merge_sidecar_prefix(",
            "pending.close_certified_sidecar_prefix(prefix)",
            "Ok(0)",
            "production close-prefix bridge must serialize",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn apply_certified_merge_sidecar_closed_prefixes(",
            ".close_certified_merge_sidecar_prefix(prefix)",
            ".retry_pending_exact_output()",
            "runner must move every lane close prefix into the worker exact-output owner before later dispatch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn service_next_certified_merge_sidecar_materialization(",
            ".next_server_request_materialization(now)",
            ".authorized_server_request_materialization()",
            "only the transport's durable fair materialization selection may cross into the terminating Kura lookup",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retryable_certified_sidecar_responder_control_target(",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "                | CertifiedMergeSidecarMessage::GenerationHint(_) => self",
            "CertifiedMergeSidecarMessage::CloseAck(_) => self",
            "worker retryable responder control must use exact reply ownership for CloseAck and GenerationHint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retryable_certified_sidecar_responder_control_target(",
            ".all(|route| matches!(&route.route, ExactTargetRoute::Reply(_))),",
            ".all(|route| matches!(&route.route, ExactTargetRoute::Topology)),",
            "worker retryable responder control must use exact reply ownership for CloseAck and GenerationHint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retains_retryable_sidecar_responder_control_for(",
            "retained.retryable_certified_sidecar_responder_control_target()\n"
            "                        == Some(candidate_target)",
            "retained.retryable_certified_sidecar_responder_control_target()\n"
            "                        .is_some()",
            "worker responder-control suppression must require an already-retained retryable control for the same semantic target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retains_retryable_sidecar_responder_control_for(",
            "== Some(candidate_target)",
            "!= Some(candidate_target)",
            "worker responder-control suppression must require an already-retained retryable control for the same semantic target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn can_enqueue(&self, fanout: &PendingExactFanout)",
            "if self.retains_retryable_sidecar_responder_control_for(fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(fanout) {",
            "lane-effect preflight must validate geometry, preflight a safely replaceable responder control, reuse identical topology ownership, retain a different rotating acquisition batch at source while ranked targets drain, retain a distinct reply duplicate at lane ownership, and otherwise charge reservation capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn can_enqueue_owned_reply_transfer(",
            "if self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "owned reply capacity preflight must consume only a same-target duplicate responder control without charging capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_validated(",
            "if self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "if false && self.retains_retryable_sidecar_responder_control_for(&fanout) {",
            "worker exact output may retain at most one retryable responder control per semantic target while preserving independent controls and ordinary progress for other targets",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_certified_merge_sidecar(",
            "self.accept_certified_merge_sidecar_generation_hint(sender, reply_route, &hint)",
            "Ok(V2LaneIngressOutcome::Rejected)",
            "lane sidecar ingress must exhaustively route the authenticated generation hint alongside every request, close, acknowledgement, and chunk variant",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "| CertifiedMergeSidecarMessage::GenerationHint(_) => None,",
            "=> None,",
            "only an immutable certified response chunk may create a writer-flush receipt from its exact route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "pub(crate) fn post_certified_merge_sidecar_with_reply_routes(",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "            | CertifiedMergeSidecarMessage::GenerationHint(_)\n"
            "            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),",
            "CertifiedMergeSidecarMessage::CloseAck(_)\n"
            "            | CertifiedMergeSidecarMessage::GenerationHint(_)\n"
            "            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_none(),",
            "worker sidecar dispatch must keep Request and Close on topology while CloseAck, GenerationHint, and Chunk retain exact reply routes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            "| CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),",
            "| CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_none(),",
            "runner sidecar dispatch must reject missing or extraneous route ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn next_effect(",
            "if take_sidecar {",
            "if !take_sidecar {",
            "lane effect peek must clone the exact fairly selected queue head",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn apply_bounded_sidecar_admissions<T, Error>(",
            "let mut applied = 0usize;",
            "return Ok(0); let mut applied = 0usize;",
            "runner exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn has_pending_exact_output(",
            "self.lock_pending_exact_output()",
            "return Ok(false); self.lock_pending_exact_output()",
            "worker exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "after_candidate_prune(merge_attempt);",
            "let _ = merge_attempt;",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "if candidate_routes.len() >= live_before_merge {",
            "if false && candidate_routes.len() >= live_before_merge {",
            "candidate pruning must retain its ownership receipt while strict or explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 2;",
            "exact-output defaults must retain the reviewed completion divisor, two Serve phase families, bounded lifecycle records, reducer batch, and three-class geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 7;",
            "the dependency-free reducer refinement must retain the reviewed maximum effect batch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core.rs",
            "const _: [(); refinement::MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT]",
            "the production embedded reducer must bind its dependency-free batch to configured exact-output geometry",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn sumeragi_v2_exact_output_shared_ownership_capacity(",
            ".checked_add(certified_request_capacity)",
            ".saturating_add(certified_request_capacity)",
            "the shared exact-output owner must reserve both bounded producers and one complete reducer batch with checked arithmetic",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn validate_sumeragi_v2_exact_output_geometry(",
            ".checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            ".saturating_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            "the geometry kernel must reject zero, multiplication overflow, and any corridor smaller than source-count times exact classes",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub fn parse(self) -> Result<actual::Root, ParseError> {",
            ".max_total_connections\n",
            ".max_connections_per_peer\n",
            "root configuration must derive the authenticated-source bound from network geometry and reject invalid canonical ingress, lifecycle, or exact-output capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "fn start_inner(",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            reply_route_source_capacity,\n"
            "        )?;",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            max_peers_per_fanout,\n"
            "        )?;",
            "production bounds protocol fanout by roster and source geometry while charging the shared pool only for the independently reserved authenticated reply sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn matches_semantic_origin(",
            "self.validate_exact() && self.first.semantic_origin.as_ref() == origin",
            "self.validate_exact()",
            "semantic-origin validation must compare the independently retained canonical request origin",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn advance_reply_cursors(",
            "if message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor {",
            "if false && (message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor) {",
            "a source attempt may advance but never reset either exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn accept_payload_chunk_with_ingress_ownership",
            "|| !ingress_ownership.matches_semantic_origin(authenticated_sender)",
            "|| false",
            "payload chunk effect consumption must reject a changed envelope or semantic origin before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn claimed_with_reply_routes_and_ingress_ownership(",
            "if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {",
            "if false {",
            "exact reply construction must attach only a validated fair-ingress carrier matching the complete per-source route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "if reply_routes.semantic_target() != &peer",
            "if false",
            "certified response emission must validate the semantic target and exact route history under one fail-stop output operation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "|| !ingress_ownership.matches_reply_routes(Some(&reply_routes))",
            "|| false",
            "certified response emission must validate the semantic target and exact route history under one fail-stop output operation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_to_peer_on_reply_routes(",
            "operation.complete();\n"
            "        Ok(())",
            "drop(operation);\n"
            "        Ok(())",
            "certified response emission preserves the complete authenticated route set until the guarded enqueue has completed",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "pub(crate) fn route_payload_chunk<R: EffectRuntime>(",
            "|| !ingress_ownership.matches_semantic_origin(&sender)",
            "|| false",
            "payload chunk routing must bind canonical bytes and semantic sender before buffering or effect mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn buffer_orphan_payload_chunk_inner(",
            "if !retained.merge_downstream(candidate) {",
            "drop(candidate); if false {",
            "orphan chunk duplicates must merge alternate source ownership without replacing canonical semantic identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_lane_message_owned(",
            "|| !ownership.matches_semantic_origin(sender.as_ref())",
            "|| false",
            "lane ingress must bind semantic origin, canonical message, and the complete source route set before service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
            "fn prepare_current_certified_serve_pre_admission(",
            "|| !ownership.matches_semantic_origin(Some(sender))",
            "|| false",
            "shared current Serve classification must bind transport ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
            "executor.can_admit_network_message_with_ingress_ownership(message, ingress_ownership)",
            "executor.runtime.can_admit_network_message_with_ingress_ownership(message, ingress_ownership)",
            "effect-executor preflight must preserve the exact fair-ingress carrier into owned runtime capacity admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn target_reservation(",
            "ExactTargetReservationKind::SidecarTopologyProgress",
            "ExactTargetReservationKind::Reliable",
            "reservation identity must isolate requester-owned topology progress from reliable reply-source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn seal(&self) -> Result<(), String>",
            ".compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)",
            ".compare_exchange(false, false, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)",
            "durable exact-output handoff must seal its unique service owner exactly once",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn seal_applied_height_output_handoff(",
            "if pending.is_pending() {",
            "if false && pending.is_pending() {",
            "final exact-output handoff must validate durable authority, atomically empty the corridor, and one-shot seal every later enqueue",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn into_retained_merge_sidecars(",
            ".is_bound_to_transport_owner(&self.exact_output_handoff_owner)",
            ".matches_predecessor_context(&self.context)",
            "lane rollover must consume only its paired service receipt for the exact predecessor artifact and immediate successor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn require_peeked_lane_work_effect(",
            "drained.ok_or(V2RunnerError::RestartRequired)",
            "drained.ok_or(V2RunnerError::Service(\"lost peek\".to_owned()))",
            "runner lane dispatch must fail stop if its guarded peek loses the exact queued owner before drain",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn matches_body_coordinates(",
            "            && self.identity.view == round.view\n",
            "",
            "exact-output ingress seam ingress::leader_wire_token_matches_body_coordinates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn classify_payload_chunk_lifecycle(",
            "            return Ok(PayloadChunkLifecycleDisposition::Retain);\n",
            "            return Ok(PayloadChunkLifecycleDisposition::Volatile);\n",
            "a live exact fetch must retain its productive chunk lifecycle",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_fetch<S: V2EffectServices>(",
            "        if round.context_id != self.context.id()\n",
            "        self.finality_completion = None;\n        if round.context_id != self.context.id()\n",
            "the durable Apply completion tombstone must have exactly the runtime and recovered authenticated installation paths",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "        self.finality_completion = Some(FinalityCompletion {\n            tag,\n",
            "        self.finality_completion = Some(FinalityCompletion {\n            tag: EventTag::new(tag.height(), 0, tag.generation()),\n",
            "runtime Apply completion must retain its exact typed owner in the finality terminal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_apply<S: V2EffectServices>(",
            "        if let Some(existing) = self.pending_applications.values().next() {\n",
            "        if false && let Some(existing) = self.pending_applications.values().next() {\n",
            "exact Apply retransmission must retain the incumbent authority and coalesce every later periodic lifecycle",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "pub(crate) fn route_payload_chunk<R: EffectRuntime>(",
            "            if self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {\n",
            "            if false && self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {\n",
            "an unmatched productive chunk must consult exact reconstructed and executor-owned lifecycle authority before buffering",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn replay_buffered_chunks<R: EffectRuntime>(",
            "        self.sweep_buffered_payload_chunk_lifecycles(executor)?;\n",
            "",
            "every orphan replay turn must sweep terminal exact chunk owners before selecting live fetch work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retire_buffered_payload_chunk_tail(",
            "                .mark_leader_wire_volatile_terminal(runtime)\n",
            "                .mark_leader_wire_volatile_terminal(runtime).and(Ok(()))\n",
            "exact-output ingress seam worker::retire_buffered_payload_chunk_tail",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn rehydrate_with_exact_geometry_after_durable_handoff(",
            "self.requeue_retained_outbound_after_height_rollover();",
            "let _ = &self;",
            "durable sidecar rehydration must preserve and requeue retained exact outbound ownership after validating the rollover authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/reply_route_retention.rs",
            "fn retain_active_owned_reply_routes_with_snapshot_hook<AfterSnapshot>(",
            "let (retained, receipt) = routes.retain_active_with_receipt();",
            "let retained = routes.retain_active();\n"
            "    let receipt = NetworkReplyRouteHistoryReceipt::default();",
            "runner pruning must retain every live source attempt and its tombstones",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
            "fn consume_prepared_dequeued_v2_ingress(",
            "        BlockMessage::KuraReplicaAdvert(_) => {\n"
            "            admit_kura_replica_advert_ingress(receiver, kura, inbound)?;\n"
            "            finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);\n"
            "        }\n",
            "",
            "KuraReplicaAdvert ingress must bypass both consensus reducers",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "const ATOMIC_PROPOSAL_FANOUT_COUNT: usize = 2;",
            "const ATOMIC_PROPOSAL_FANOUT_COUNT: usize = 2;",
            "const ATOMIC_PROPOSAL_FANOUT_COUNT: usize = 1;",
            "one atomic Proposal admission may drive exactly its proposal and chunk fanouts",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn new(\n"
            "        shared_ownership_unit_capacity: usize,\n"
            "        max_messages_per_fanout: usize,",
            ".checked_mul(ATOMIC_PROPOSAL_FANOUT_COUNT)",
            ".saturating_mul(ATOMIC_PROPOSAL_FANOUT_COUNT)",
            "exact-output construction must deterministically checked-multiply the two-fanout atomic Proposal drive budget by the larger protocol service bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_bounded_with_ack<Attempt>(",
            "self.drive_with_budget_ack(self.drive_attempt_budget, attempt)",
            "self.drive_with_budget_ack(usize::MAX, attempt)",
            "the production exact-output driver must consume the checked atomic Proposal drive budget",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "pub(crate) fn start(\n",
            "            body_store,\n            None,\n            state,",
            "            body_store,\n            Some(body_store.instance_identity()),\n            state,",
            "ordinary startup must explicitly omit recovered Certified-Serve payload-store identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "fn start_inner(\n",
            "            lifecycle_payload_store_identity,\n            fetches: BTreeMap::new(),",
            "            lifecycle_payload_store_identity: None,\n            fetches: BTreeMap::new(),",
            "the live worker must retain both exact recovered lifecycle store identities",
        ),
    ),
)
def test_exact_output_production_source_mutations_fail_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str | tuple[str, ...],
) -> None:
    module = load_checker()
    exact_output_production_fixture(tmp_path)

    if region_marker == "pub(crate) fn accept_payload_chunk_with_ingress_ownership":
        _ingress_effects_mutations_survive_digest_refresh(
            tmp_path / "ingress-effects-semantics"
        )

    if region_marker == "fn classified_with_route_history(":
        _worker_ack_mutations_survive_digest_refresh(tmp_path / "worker-ack-semantics")

    if region_marker == "fn start_inner(" and old == ".map(|entry| entry.validator.clone())":
        _worker_ownership_mutations_survive_digest_refresh(tmp_path / "worker-ownership-semantics")

    if (
        region_marker == "fn consume_prepared_dequeued_v2_ingress("
        and old == "services.post_durable_history_response_on_reply_routes_with_permit("
    ):
        _ordinary_ingress_consumer_mutations_survive_digest_refresh(
            tmp_path / "ordinary-consumer-semantics"
        )
        _decided_body_serve_mutations_survive_digest_refresh(
            tmp_path / "decided-body-serve-semantics"
        )

    if (
        relative_path == "crates/iroha_config/src/parameters/user.rs"
        and region_marker == "pub fn parse(self) -> Result<actual::Root, ParseError> {"
    ):
        _root_parse_geometry_mutations_survive_digest_refresh(
            tmp_path / "root-geometry-semantics"
        )

    if (
        region_marker == "fn new_with_output_guard_and_transport_inner("
        and old == "MergeSidecarTransport::open_durable_with_server_stream_capacity("
    ):
        _lane_output_mutations_survive_digest_refresh(tmp_path / "lane-output-semantics")

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    next_item = re.search(
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?fn[ \t]+",
        source[region_start + len(region_marker) :],
    )
    if next_item is not None:
        next_item_start = region_start + len(region_marker) + next_item.start()
        assert mutation < next_item_start, (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    expected_errors = (
        [error_fragment]
        if isinstance(error_fragment, str)
        else list(error_fragment)
    )
    if region_marker == "fn derive_server_request_capacities(":
        expected_errors.extend(
            _apply_exact_output_non_runtime_extended_mutations(
                tmp_path, module, monkeypatch
            )
        )

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)
    assert all(
        any(expected_error in error for error in errors)
        for expected_error in expected_errors
    ), errors

def _lane_output_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Keep requester and rollover authority checks effective after resealing."""
    module = load_checker()
    relative = Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs")
    canonical = ROOT_DIR / relative
    assert canonical.is_file() and not canonical.is_symlink()
    source = canonical.read_text(encoding="utf-8")
    names = (
        "new_with_output_guard_and_transport_inner",
        "accept_certified_merge_sidecar_request",
        "service_next_certified_merge_sidecar_materialization",
        "durable_lane_rollover_authority",
    )
    items, digests = {}, {}
    errors: list[str] = []
    for name in names:
        item = module._require_qualified_rust_item(
            canonical, source, "V2LaneWorkAdapter", name, errors,
            "canonical lane-output production owner",
            expected_attributes=("#[allow(clippy::too_many_arguments)]",)
            if name == names[0] else (),
        )
        assert item is not None and not errors, errors
        table = (module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256
                 if name == names[3] else module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256)
        key = name if name == names[3] else f"V2LaneWorkAdapter::{name}"
        module._require_rust_item_token_sha256(canonical, item, table[key], name, errors)
        items[name], digests[name] = item, table[key]
    module._require_lane_output_reconciled_source_contracts(
        canonical, {f"V2LaneWorkAdapter::{name}": item for name, item in items.items()},
        items, errors,
    )
    assert not errors, errors
    constructor, request, materialize, rollover = names
    request_error = "bounded exact historical requester before allocation"
    materialize_error = "exact metadata, authenticated historical finality, requester membership and local custody"
    rollover_error = "must keep the predecessor active until each winning lane has an exact durable certificate"
    mutations = (
        (constructor, "empty-hints", "obsolete_merge_sidecar_generation_hints: BTreeMap::new(),", "obsolete_merge_sidecar_generation_hints: retained_hints,", "initialize empty obsolete generation hints"),
        (constructor, "instrumentation-test-only", "#[cfg(test)]\n            merge_candidate_validation_checks", "merge_candidate_validation_checks", "validation instrumentation test-only"),
        (constructor, "instrumentation-zero", "merge_candidate_validation_checks: std::cell::Cell::new(0),", "merge_candidate_validation_checks: std::cell::Cell::new(1),", "validation instrumentation test-only"),
        (constructor, "empty-memo", "validated_merge_execution_candidate: None,", "validated_merge_execution_candidate: retained_candidate,", "production memo empty"),
        (request, "semantic-target", "reply_route.semantic_target() != &sender", "reply_route.semantic_target() == &sender", request_error),
        (request, "active-route", "!reply_route.is_active()", "false", request_error),
        (request, "current-membership", "let sender_is_current = self.frozen_roster_contains(&sender);", "let sender_is_current = true;", request_error),
        (request, "requester-identity", "|| request.requester != sender", "|| false", request_error),
        (request, "request-version", "request.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1", "false", request_error),
        (request, "request-id", "|| request.request_id != request.canonical_request_id()", "|| false", request_error),
        (request, "bounded-request", "u64::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES).unwrap_or(u64::MAX)", "u64::MAX", request_error),
        (request, "predecessor-read-error", ".immediate_predecessor_sidecar_requesters()?", ".immediate_predecessor_sidecar_requesters().ok().flatten()", request_error),
        (request, "historical-auth", "self.exact_historical_lane_sidecar_requester(&request, &sender)?", "true", request_error),
        (request, "historical-auth-error", "self.exact_historical_lane_sidecar_requester(&request, &sender)?", "self.exact_historical_lane_sidecar_requester(&request, &sender).unwrap_or(false)", request_error),
        (request, "outsider-reject", "if !sender_is_predecessor && !sender_is_lane_validator {", "if !sender_is_predecessor && false {", request_error),
        (request, "corridor-capacity", "historical_streams >= wire::MAX_VALIDATORS_PER_HEIGHT", "historical_streams > wire::MAX_VALIDATORS_PER_HEIGHT", "complete-committee stream bound"),
        (request, "stream-generation", ".would_allocate_current_server_stream(&sender, request.service_generation)", ".would_allocate_current_server_stream(&sender, request.service_generation.saturating_add(1))", "complete-committee stream bound"),
        (materialize, "corrupt-read-as-absence", "return self.consensus_storage_read(Err(error));", "return Ok(false);", "preserve admitted ownership on corrupt local reads"),
        (materialize, "corrupt-read-retire", "return self.consensus_storage_read(Err(error));", "self.merge_sidecars.retire_unmaterialized_server_request(&requester, &request)?;\n                return self.consensus_storage_read(Err(error));", "preserve admitted ownership on corrupt local reads"),
        (materialize, "exact-reference", "request.reference_digest == certified_merge_reference_digest(&reference)", "true", materialize_error),
        (materialize, "lane-membership", "self.finalized_merge_active_lane_committee_contains(&entry, &requester)", "true", materialize_error),
        (materialize, "finality-requester", "Some(&requester),", "None,", materialize_error),
        (materialize, "custody", "&& local_is_authenticated_custodian;", ";", materialize_error),
        (materialize, "holder-reject", "if !local_is_holder {", "if false {", materialize_error),
        (materialize, "finality-error", "            )?\n            && local_is_authenticated_custodian;", "            ).unwrap_or(false)\n            && local_is_authenticated_custodian;", materialize_error),
        (rollover, "canonical-lossy-read", "self.canonical_block_body(height)?", "self.kura.get_block(height)", "exact canonical block"),
        (rollover, "strict-certificate", "self.kura.read_lane_completion_certificate(", "self.kura.read_certified_lane_block_artifact(", rollover_error),
        (rollover, "strict-payload", "self.kura.read_lane_completion_autonomous_artifact(", "self.kura.read_autonomous_lane_block_artifact(", rollover_error),
        (rollover, "replica-network", "                                network_id,\n                                self.context.epoch,", "                                wire::NetworkId::default(),\n                                self.context.epoch,", rollover_error),
        (rollover, "replica-height", "                                descriptor.lane_block_height,", "                                descriptor.proposal_height,", rollover_error),
        (rollover, "replica-on-partial", "(None, None) => {", "(None, _) => {", rollover_error),
        (rollover, "partial-private-omit", "(Some(_), None) | (None, Some(_)) => return Ok(None),", "(Some(_), None) | (None, Some(_)) => continue,", rollover_error),
        (rollover, "replica-absent-omit", "let Some(replica) = replica else {\n                            return Ok(None);", "let Some(replica) = replica else {\n                            continue;", rollover_error),
        (rollover, "ordinary-absent-omit", "let Some(durable) = private_durable else {\n                    return Ok(None);", "let Some(durable) = private_durable else {\n                    continue;", rollover_error),
        (rollover, "execution-role", "if autonomous_certificate != autonomous_payload.is_some() {", "if false {", rollover_error),
        (rollover, "strict-receipt", "self.kura.read_lane_completion_receipt(proposal)", "self.kura.read_lane_completion_receipt_unchecked(proposal)", rollover_error),
        (rollover, "receipt-finality", "|| receipt.application_block_hash != finality_artifact.block_hash", "|| false", "bind every winner to the exact applied artifact"),
        (rollover, "exact-payload-proposal", "if payload.origin_proposal != *proposal {", "if false {", "one exact ordinary or autonomous durable witness per winner"),
        (rollover, "complete-winner-set", "durable_sessions.insert(proposal.proposal_hash, source);", "let _ = source;", "one exact ordinary or autonomous durable witness per winner"),
    )
    for index, (name, case, old, new, expected) in enumerate(mutations):
        assert items[name].source.count(old) == 1, (case, old)
        _assert_lane_output_rehashed_mutant(
            tmp_path / f"{index:02d}-{case}", module, relative, items,
            name, items[name].source.replace(old, new, 1), digests[name], expected,
        )
    # Move complete statements while preserving every reviewed guard token.
    request_source = items[request].source
    admission_start = request_source.index("        let now = Instant::now();")
    admission_end = request_source.index("        let ingress_selected =", admission_start)
    admission = request_source[admission_start:admission_end]
    order_cases = (
        (constructor, "construction-early-complete", "        construction.complete();", "        let adapter = Self {", False, "complete its fail-stop operation only after initializing every owner"),
        (request, "admission-before-auth", admission, "        let sender_is_current =", False, "authentication and stream bounds must precede durable admission"),
        (materialize, "bytes-before-auth", "        let selected_reply_route = reply_route.clone();", "        let reference = CertifiedMergeLedgerReference::new(&entry);", False, None),
        (rollover, "publish-before-winner-validation", "            durable_sessions.insert(proposal.proposal_hash, source);", "            if durable.proposal != *proposal", False, "strict rollover reads and complete winner validation must precede durable authority publication"),
    )
    for name, case, moving, target, after, expected in order_cases:
        current = items[name].source
        if name == materialize:
            # Move the complete enqueue match (the function's final expression)
            # above metadata/finality validation, leaving those guards present.
            begin = current.index(moving)
            moving = current[begin:current.rfind("    }")]
            expected = "complete serving authority must precede response bytes"
        assert current.count(moving) == current.count(target) == 1, case
        changed = current.replace(moving, "", 1)
        changed = changed.replace(target, target + "\n" + moving if after else moving + "\n" + target, 1)
        _assert_lane_output_rehashed_mutant(
            tmp_path / case, module, relative, items, name, changed,
            digests[name], expected,
        )


def _assert_lane_output_rehashed_mutant(
    root: Path, module, relative: Path, baseline_items: dict,
    name: str, source: str, baseline_digest: str, expected: str,
) -> None:
    path = root / relative
    path.parent.mkdir(parents=True)
    baseline = baseline_items[name]
    wrapper = "impl V2LaneWorkAdapter {\n" + "\n".join(baseline.attributes) + "\n" + source + "\n}\n"
    path.write_text(wrapper, encoding="utf-8")
    errors: list[str] = []
    item = module._require_qualified_rust_item(
        path, wrapper, "V2LaneWorkAdapter", name, errors,
        "independently rehashed lane-output owner",
        expected_attributes=baseline.attributes,
    )
    assert item is not None and not errors, errors
    digest = module._rust_item_token_sha256(item)
    assert digest != baseline_digest
    table = (module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256
             if name == "durable_lane_rollover_authority" else module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256)
    key = name if name == "durable_lane_rollover_authority" else f"V2LaneWorkAdapter::{name}"
    original = table[key]
    table[key] = digest
    try:
        module._require_rust_item_token_sha256(path, item, table[key], name, errors)
        selected = dict(baseline_items, **{name: item})
        module._require_lane_output_reconciled_source_contracts(
            path, {f"V2LaneWorkAdapter::{owner}": value for owner, value in selected.items()},
            selected, errors,
        )
    finally:
        table[key] = original
    (root / "source-rehash.json").write_text(json.dumps({
        "owner": name, "baseline_token_sha256": baseline_digest,
        "mutant_token_sha256": digest, "expected_semantic_error": expected,
        "errors": errors,
    }, indent=2) + "\n", encoding="utf-8")
    assert any(expected in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


def _root_parse_geometry_mutations_survive_digest_refresh(tmp_path: Path) -> None:
    """Keep parser guards and publication order authoritative after a fresh seal."""
    module = load_checker()
    relative = Path("crates/iroha_config/src/parameters/user.rs")
    path = ROOT_DIR / relative
    errors: list[str] = []
    source = path.read_text(encoding="utf-8")
    structural = module.mask_rust_comments_and_literals(source)
    marker = "    pub fn parse(self) -> Result<actual::Root, ParseError> {"
    assert structural.count(marker) == 1
    start = structural.index(marker)
    body_start = structural.index("{", start)
    depth, end = 1, body_start + 1
    while depth:
        depth += (structural[end] == "{") - (structural[end] == "}")
        end += 1
    # Select this complete item without parsing every unrelated `parse`
    # method in user.rs. Context and attributes still come from the actual
    # full production source, using the checker's canonical lexical helpers.
    delimiter_context = module._rust_delimiter_context(structural, start)
    item = module.RustItem(
        name="parse", line=source.count("\n", 0, start) + 1,
        source=source[start:end], body=source[body_start + 1:end - 1],
        structural_source=structural[start:end],
        brace_context=module._rust_brace_context(structural, start),
        delimiter_context=delimiter_context,
        attributes=module._leading_rust_attributes(source, structural, start),
        ancestor_inner_attributes=module._rust_ancestor_inner_cfg_attributes(
            source, structural, start, delimiter_context,
        ),
    )
    baseline_digest = module._PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256["user::Root::parse"]
    module._require_rust_item_token_sha256(path, item, baseline_digest, "Root::parse", errors)
    module._require_root_parse_exact_output_geometry_contract(path, item, errors)
    assert not errors, errors
    geometry = "reject invalid canonical ingress, lifecycle, or exact-output capacity"
    mutations = (
        ("network-capacity", ".max_total_connections\n", ".max_connections_per_peer\n", geometry),
        ("network-derived-fallback", ".or(lane_profile.derived_limits().max_total_connections)", "", geometry),
        ("network-default-fallback", "lane_profile.defaults().max_total_connections,", "1,", geometry),
        ("trusted-full-fanout", "if remote_trusted_peer_count > reply_source_capacity {", "if false {", geometry),
        ("nonvalidator-source-bound", "if sumeragi.queues.authenticated_non_validator_sources.get() > reply_source_capacity {", "if false {", geometry),
        ("completion-reserve", "/ defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR", "* defaults::sumeragi::V2_RUNTIME_COMPLETION_RESERVE_DIVISOR", geometry),
        ("complete-validator-roster", "trusted_peers.value().validator_roster_len()", "trusted_peers.value().others.len()", geometry),
        ("canonical-message-minimum", "Some(required_bodies) if sumeragi.queues.bodies.get() < required_bodies", "Some(required_bodies) if false", geometry),
        ("canonical-byte-minimum", "if sumeragi.queues.body_bytes.get() < required_body_bytes", "if false", geometry),
        ("per-source-wire-budget", "let body_source_bytes = sumeragi.queues.body_source_bytes.get();", "let body_source_bytes = 1;", geometry),
        ("lifecycle-source-bound", "actual::sumeragi_v2_lifecycle_capacity_geometry(\n                validator_roster_len,", "actual::sumeragi_v2_lifecycle_capacity_geometry(\n                0,", geometry),
        ("shared-output-source-bound", "                    shared_capacity,\n                    reply_source_capacity,", "                    shared_capacity,\n                    1,", geometry),
        ("collect-sccp-error", "emit_torii_config_error(&mut emitter, message);", "let _ = message;", "validate SCCP/Norito limits"),
        ("exact-sccp-input", "&self.torii.sccp_replay_archive, &self.norito", "&self.torii.sccp_replay_archive, &Norito::default()", "validate SCCP/Norito limits"),
        ("direct-telemetry-profile", "actual::TelemetryProfile::from(self.telemetry_profile)", "actual::TelemetryProfile::Disabled", "sole direct telemetry profile"),
        ("retired-master", "let telemetry_profile =", "let telemetry_enabled = true;\n        let telemetry_profile =", "must not restore retired telemetry"),
        ("retired-redaction", "let telemetry_profile =", "let telemetry_redaction = true;\n        let telemetry_profile =", "must not restore retired telemetry"),
        ("parsed-sumeragi", "if let Some(sumeragi) = sumeragi.as_ref() {", "if let Some(sumeragi) = None {", "geometry for the parsed Sumeragi"),
        ("collect-pipeline-error", "let pipeline = self.pipeline.parse(&mut emitter);", "let pipeline = self.pipeline.parse();", "pipeline parsing must report"),
        ("propagate-collective-errors", "emitter.into_result()?;", "emitter.into_result();", "propagate collective validation failure"),
        ("apply-storage-budget", "root.apply_storage_budget();", "", "apply the storage budget"),
    )
    # Every retained mutant contains the complete Root item in its real impl
    # context. The baseline above authenticates this item against the full
    # canonical source; no fixture digest can substitute for that provenance.
    for index, (case, old, new, expected) in enumerate(mutations):
        assert old in item.source, case
        mutated_source = item.source.replace(old, new, 1)
        _assert_root_parse_geometry_mutant(
            tmp_path / f"{index:02d}-{case}", module, relative, item,
            mutated_source, baseline_digest, expected,
        )
    order_cases = (
        (
            "pipeline-after-validation", "let pipeline = self.pipeline.parse(&mut emitter);",
            "emitter.into_result()?;",
            "must precede configuration publication in the reviewed order",
        ),
        (
            "validation-after-root-construction", "emitter.into_result()?;",
            "root.apply_storage_budget();",
            "must precede configuration publication in the reviewed order",
        ),
        (
            "sccp-after-torii-consumption",
            "if let Err(message) =\n"
            "            validate_sccp_replay_archive_norito_limit(&self.torii.sccp_replay_archive, &self.norito)\n"
            "        {\n            emit_torii_config_error(&mut emitter, message);\n        }",
            "let (torii, live_query_store) = self.torii.parse(&mut emitter, parsed_sorafs);",
            "validate SCCP/Norito limits through the collective emitter before consuming Torii",
        ),
    )
    for case, moving, after, expected in order_cases:
        assert item.source.count(moving) == item.source.count(after) == 1
        mutated_source = item.source.replace(moving, "", 1)
        # The validation/root case deliberately moves the collective error
        # boundary beyond the already constructed Root while retaining tokens.
        mutated_source = mutated_source.replace(after, after + "\n        " + moving, 1)
        _assert_root_parse_geometry_mutant(
            tmp_path / case, module, relative, item, mutated_source,
            baseline_digest, expected,
        )


def _assert_root_parse_geometry_mutant(
    root: Path, module, relative: Path, baseline, source: str,
    baseline_digest: str, expected: str,
) -> None:
    path = root / relative
    path.parent.mkdir(parents=True)
    wrapper = "impl Root {\n" + "\n".join(baseline.attributes) + "\n" + source + "\n}\n"
    path.write_text(wrapper, encoding="utf-8")
    errors: list[str] = []
    item = module._require_qualified_rust_item(
        path, wrapper, "Root", "parse", errors, "rehashed Root parser mutation",
        expected_attributes=("#[allow(clippy::too_many_lines)]",),
    )
    assert item is not None and not errors, errors
    digest = module._rust_item_token_sha256(item)
    assert digest != baseline_digest
    original = module._PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256["user::Root::parse"]
    module._PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256["user::Root::parse"] = digest
    try:
        module._require_rust_item_token_sha256(
            path, item,
            module._PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256["user::Root::parse"],
            "Root::parse", errors,
        )
        module._require_root_parse_exact_output_geometry_contract(path, item, errors)
    finally:
        module._PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256["user::Root::parse"] = original
    (root / "source-rehash.json").write_text(json.dumps({
        "baseline_token_sha256": baseline_digest, "mutant_token_sha256": digest,
        "expected_semantic_error": expected, "errors": errors,
    }, indent=2) + "\n", encoding="utf-8")
    assert any(expected in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors


def _apply_exact_output_non_runtime_extended_mutations(
    tmp_path: Path, module, monkeypatch: pytest.MonkeyPatch
) -> list[str]:
    mutations = (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "|| !self.recovered_decision_fetch_request_index_is_exact_and_empty()",
            "|| false",
            "runtime Apply completion must reject a second terminal or recovered-Fetch overlap",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            "|| !self.recovered_decision_fetch_request_index_is_exact_and_empty()",
            "|| false",
            "lifecycle Decision Apply completion must not overtake retained work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            "&& self.recovered_decision_fetch_request_index_is_exact_and_empty()",
            "&& true",
            "lifecycle Decision Apply finality must authenticate lineage, recovery, height, artifact, receipt, and drained runtime",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn derive_server_request_capacities(",
            "|| server_stream_capacity > MAX_CERTIFIED_MERGE_SERVER_STREAMS",
            "|| false",
            "must enforce the protocol roster bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn lifecycle_protocol_max_snapshot_bytes(",
            "MAX_CERTIFIED_MERGE_SERVER_STREAMS,",
            "self.server_stream_capacity,",
            "must reserve the maximum bounded responder roster",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn rehydrate_after_lifecycle_restore(",
            "if server_stream_capacity < self.server_stream_capacity {",
            "if false && server_stream_capacity < self.server_stream_capacity {",
            "permit only monotonic same-roster capacity expansion",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "        Self {\n            session_capacity,",
            "historical_recovery_retry_floor,",
            "historical_recovery_retry_ceiling,",
            "lane limits must retain the exact configured reply-source geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn new_with_output_guard_and_transport(",
            ".zip(verified_context.proofs_of_possession())",
            ".zip(std::iter::empty())",
            "must derive its context and frozen validator proofs from one verified carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "let sidecar_server_stream_capacity =\n"
            "            merge_sidecar_server_stream_capacity(sidecar_server_roster.len());",
            "let sidecar_server_stream_capacity = 1;",
            "must derive the canonical responder roster",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn hydrate_canonical_lane_artifacts(",
            "&& !self.lane_sessions.contains_proposal(proposal)",
            "&& false",
            "must bound and authorize every exact historical recovery proposal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn preflight_effect_insertion(",
            "if !predecessor_ready {",
            "if false {",
            "ordinary lane effect preflight must retain applied-predecessor readiness, exact identity, bounded capacity, and complete reply-route history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn validate_winning_lane_output(",
            "validate_lane_vote_for_proposal(vote, proposal)?;",
            "let _ = (vote, proposal);",
            "must authenticate both standalone votes and aggregate certificates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "let Some(durable) = private_durable else {\n                    return Ok(None);\n                };",
            "let Some(durable) = private_durable else {\n                    continue;\n                };",
            "must keep the predecessor active until each winning lane has an exact durable certificate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "durable_sessions.insert(proposal.proposal_hash, source);",
            "let _ = source;",
            "must preserve one exact ordinary or autonomous durable witness per winner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "struct FinalityCompletion {",
            "ownership: FinalityCompletionOwner,",
            "ownership: Option<FinalityCompletionOwner>,",
            "durable Apply tombstone must retain the exact reducer incarnation tag",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "            ownership: FinalityCompletionOwner::Runtime(ownership),\n        });",
            "            ownership: FinalityCompletionOwner::LifecycleDecisionApply(\n"
            "                LifecycleDecisionApplyDispatchKeyV1::for_test(1, 1),\n            ),\n        });",
            "durable Apply completion must retain the exact tag",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn begin_apply<S: V2EffectServices>(",
            ".same_commit_decision(certificate.as_ref())",
            ".same_commit_decision(existing.task.certificate.as_ref())",
            "exact Apply retransmission must retain the incumbent authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn complete_application<S: V2EffectServices>(",
            "self.preflight_pending_application_owner(completion.work_id, pending)",
            "Ok(())",
            "runtime Apply completion must preflight its exact separately retained owner before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            ".as_ref()\n                .is_some_and(|owner| {",
            ".as_ref()\n                .is_none_or(|owner| {",
            "lifecycle Decision Apply completion must authenticate its exact live owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            "LifecycleDecisionApplyLineageV1::Recovered => {\n                self.live_lifecycle_decision_apply.is_none()",
            "LifecycleDecisionApplyLineageV1::Recovered => {\n                self.live_lifecycle_decision_apply.is_some()",
            "direct recovered Decision Apply completion must prove live-owner non-substitution",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            "authority.exactly_matches_pending_kura_recovery(&self.context, evidence)",
            "true",
            "lifecycle Decision Apply completion must bind optional interrupted-tip evidence to the exact recovered lineage",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            "|| !pending_recovery_is_exact",
            "|| false",
            "lifecycle Decision Apply completion must authenticate exact pending-Kura recovery evidence",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
            "|| self.finality_completion.is_some()",
            "|| false",
            "lifecycle Decision Apply completion must not overtake retained work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            ".take()\n                .is_some_and(|owner| {",
            ".take()\n                .is_none_or(|owner| {",
            "lifecycle Decision Apply finality must consume its exact live owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            "LifecycleDecisionApplyLineageV1::Recovered => {\n                self.live_lifecycle_decision_apply.is_none()",
            "LifecycleDecisionApplyLineageV1::Recovered => {\n                self.live_lifecycle_decision_apply.is_some()",
            "direct recovered Decision Apply finality must prove live-owner non-substitution",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            "dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered",
            "true",
            "lifecycle Decision Apply finality must bind pending-Kura evidence only to the recovered dispatched lineage",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            "&& pending_recovery_is_exact",
            "&& true",
            "lifecycle Decision Apply finality must authenticate exact pending-Kura recovery evidence",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(in crate::sumeragi) fn commit_lifecycle_decision_apply_finality(",
            "&& dispatch_key.matches_height_context(&self.context)",
            "&& true",
            "lifecycle Decision Apply finality must authenticate lineage, recovery, height, artifact, receipt, and drained runtime",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(in crate::sumeragi) fn start_with_apply_service(",
            "!apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
            "false",
            "recovered startup must authenticate State, Kura, context, and proof roster before sharing the constructor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(in crate::sumeragi) fn activate_effect_completion_observer(",
            "_permit: ProductionV2CompletionObserverActivationPermitV1,",
            "_permit: (),",
            "completion observer activation must require its opaque permit and arm fail-stop first",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn new_with_output_guard_and_transport_inner(",
            "lifecycle_decision_apply_sidecar_waits: BTreeSet::new(),\n"
            "            rejected_lifecycle_decision_apply_sidecars: BTreeMap::new(),",
            "lifecycle_decision_apply_sidecar_waits: BTreeSet::new(),\n"
            "            rejected_lifecycle_decision_apply_sidecars: BTreeMap::from([]),",
            "lane construction must initialize distinct lifecycle Apply wait and rejection owners",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(in crate::sumeragi) fn defer_missing_lifecycle_decision_apply_sidecar(",
            "self.lifecycle_decision_apply_sidecar_waits.insert(entry_hash);",
            "self.lifecycle_decision_apply_sidecar_waits.remove(&entry_hash);",
            "lifecycle Apply sidecar deferral must retain only live wait ownership",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn defer_decided_block(",
            "InboundPriority::Decided,\n            false,",
            "InboundPriority::Decided,\n            true,",
            "ordinary decided sidecars must remain executor-census owned",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn defer_lifecycle_decided_block(",
            "InboundPriority::Decided,\n            true,",
            "InboundPriority::Decided,\n            false,",
            "recovered Apply sidecars must enter the lifecycle-owned corridor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn defer_block_with_priority(",
            "if lifecycle_owned {\n                    carrier.lifecycle_owned = true;\n                }",
            "if false {\n                    carrier.lifecycle_owned = true;\n                }",
            "repeated exact registration must monotonically promote lifecycle ownership",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn retain_pending_blocks(",
            "(!carrier.lifecycle_owned && !pending_blocks.contains(&carrier.hash))",
            "!pending_blocks.contains(&carrier.hash)",
            "cleanup preflight must preserve live lifecycle-owned carriers while recognizing committed height",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn retain_pending_blocks(",
            "carrier.lifecycle_owned || pending_blocks.contains(hash)",
            "pending_blocks.contains(hash)",
            "cleanup must retain lifecycle-owned carriers only until their height commits",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn sweep_buffered_payload_chunk_lifecycles<R: EffectRuntime>(",
            "                Err(error) => {\n"
            "                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {\n"
            "                        manifest_hash: cursor.manifest_hash,\n"
            "                        chunk_offset: cursor.chunk_offset.saturating_add(1),\n"
            "                    });",
            "                Err(error) => {\n"
            "                    self.orphan_lifecycle_sweep_cursor = None;",
            "buffered lifecycle sweep must preserve every nonterminal or unclassifiable exact owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "if !self.rollover_claim.accepts_superseded_reply_delivery() {",
            "if false {",
            "explicitly authorized superseded history produces a typed merge receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_observed_with_receipt(&candidate_routes)",
            ".merge_with_receipt(&candidate_routes)",
            "worker-side observed-history reconciliation seams",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_downstream_with_observed_receipt(candidate, receipt)",
            ".merge_downstream_with_strict_receipt(candidate, receipt)",
            "must consume the typed strict or superseded receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "fn applied_height_reconstruction_covers(",
            "if proposal_height >= artifact.height {",
            "if false {",
            "lane output retirement must require exact typed durable or independently readable historical lane authority",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "let covered = match envelope.as_message() {",
            "lane_output_is_covered(lane_message)?",
            "false",
            "lane output retirement must route every typed lane class through the reviewed authority classifier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "pub(super) fn run_non_pending_lifecycle_loop(",
            "            exact_output_service_owner,\n        );",
            "            exact_output_service_owner.clone(),\n        );",
            "lifecycle construction must move the unique service owner into the launch corridor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "pub(super) fn run_pending_kura_lifecycle_height(",
            "        exact_output_service_owner,\n    );",
            "        exact_output_service_owner.clone(),\n    );",
            "lifecycle construction must move the unique service owner into the launch corridor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "pub(super) fn run_non_pending_lifecycle_loop(",
            "            retransmit_interval,\n            round_timeout,",
            "            Duration::ZERO,\n            Duration::ZERO,",
            "lifecycle construction must pass the exact P2P source geometry into lane work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "pub(super) fn run_pending_kura_lifecycle_height(",
            "        retransmit_interval,\n        round_timeout,",
            "        Duration::ZERO,\n        Duration::ZERO,",
            "lifecycle construction must pass the exact P2P source geometry into lane work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "pub(super) fn run_non_pending_lifecycle_loop(",
            "config.role == NodeRole::Validator,",
            "local_validator.is_some(),",
            "each lane adapter must be constructed from the same configured role after startup reconciliation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "pub(super) fn run_pending_kura_lifecycle_height(",
            "config.role == NodeRole::Validator,",
            "local_validator.is_some(),",
            "each lane adapter must be constructed from the same configured role after startup reconciliation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "pub(super) fn run_non_pending_lifecycle_loop(",
            "lifecycle_process_generation.clone(),",
            "None,",
            "each lane adapter must receive the same process-lifetime generation claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "pub(super) fn run_pending_kura_lifecycle_height(",
            "lifecycle_process_generation.clone(),",
            "None,",
            "each lane adapter must receive the same process-lifetime generation claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
            "const fn accepts_superseded_reply_delivery(&self) -> bool {",
            "| Self::DurableCertifiedBodyResponse { .. }",
            "| Self::DurableLaneCertificateResponse { .. }",
            "superseded reply history must be limited to durable global response claims",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "fn matches_apply(",
            "matches!(&self.ownership, FinalityCompletionOwner::Runtime(retained) if retained == ownership)",
            "true",
            "durable Apply tombstone equality must bind the runtime incarnation, tag, finality decision, and Kura receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_historical_lane_output_source_hash(",
            "if durable.proposal.proposal_hash != proposal_hash {",
            "if false {",
            "historical lane retirement must authenticate the exact durable proposal and bind its output hash",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn durable_historical_lane_verification_pops(",
            "|| hint.proposal_block_hash != finality.block_hash",
            "|| false",
            "historical lane verification must source alternate signer PoPs from the frozen finality roster",
        ),
    )
    diagnostics = []
    for relative_path, marker, old, new, diagnostic in mutations:
        path = tmp_path / relative_path
        source = path.read_text(encoding="utf-8")
        region = source.find(marker)
        assert region >= 0, (relative_path, marker)
        offset = source.find(old, region)
        assert offset >= 0, (relative_path, marker, old)
        path.write_text(
            source[:offset] + new + source[offset + len(old) :],
            encoding="utf-8",
        )
        diagnostics.append(diagnostic)
    monkeypatch.setattr(module, "_require_rust_item_token_sha256", lambda *args: None)
    return diagnostics

@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "proposal_predecessor_is_ready_for_progress",
            "|| self.autonomous_payload_is_expected_for(proposal)?",
            "|| false",
            "lane predecessor readiness must dispatch autonomous and ordinary proofs",
        ),
        (
            "proposal_predecessor_is_ready_for_progress",
            "|| finalized_observer",
            "|| false",
            "lane predecessor readiness must dispatch autonomous and ordinary proofs",
        ),
        (
            "proposal_predecessor_is_ready_for_progress",
            "!self.local_can_own_autonomous_payload(proposal)",
            "self.local_can_own_autonomous_payload(proposal)",
            "lane predecessor readiness must dispatch autonomous and ordinary proofs",
        ),
        (
            "proposal_predecessor_is_ready_for_progress",
            "})?\n                .is_some()",
            "})?\n                .is_none()",
            "lane predecessor readiness must dispatch autonomous and ordinary proofs",
        ),
        (
            "proposal_predecessor_is_ready_for_progress",
            ".certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)",
            ".certified_lane_block_predecessor_is_applied_or_snapshot_anchored(proposal)",
            "lane predecessor readiness must dispatch autonomous and ordinary proofs",
        ),
        (
            "persist_anchored_sessions",
            "if !self.proposal_predecessor_is_ready_for_progress(&session.proposal)? {",
            "if false {",
            "anchored lane persistence must retain a certified successor",
        ),
        (
            "reconstruct_durable_lane_certificate",
            "if !self\n            .proposal_predecessor_is_ready_for_progress(proposal)\n            .map_err(|_| ())?\n        {",
            "if false {",
            "lane recovery reconstruction must not emit a successor certificate",
        ),
        (
            "preflight_effect_insertion",
            "if !predecessor_ready {",
            "if false {",
            "lane effect admission must reject every fresh consensus output",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "if historical_records.len() > hydration_capacity {",
            "if false {",
            "historical lane hydration must bound and authenticate every recovery record",
            id="historical-record-count-bound",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "validate_historical_autonomous_lane_recovery_record(\n"
            "                self.state.as_ref(),\n"
            "                self.kura.as_ref(),\n"
            "                &record,\n"
            "            )\n"
            "            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;",
            "let _ = &record;",
            "historical lane hydration must bound and authenticate every recovery record",
            id="historical-record-authentication",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "if committed != 0 && committed != record.reservation_group.ordered_keys.len() {",
            "if false {",
            "historical lane hydration must reject partially committed FIFO groups",
            id="historical-partial-fifo",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "if certified.proposal != *proposal",
            "if certified.proposal.proposal_hash != proposal.proposal_hash",
            "historical lane hydration must authenticate the entire certified proposal",
            id="historical-entire-certified-proposal",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &record => {}",
            "std::collections::btree_map::Entry::Occupied(entry) if true => {}",
            "historical lane hydration must preserve immutable record identity",
            id="historical-immutable-record",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            ".insert_recovered_proposals(&raw_proposals)",
            ".insert_recovered_proposals(&[])",
            "one complete bounded recovery batch before publishing payloads or historical READY",
            id="historical-capacity-replacement-owner",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "self.lane_sessions\n"
            "            .insert_recovered_proposals(&raw_proposals)",
            "if self.lane_sessions.len() >= hydration_capacity {\n"
            "            return Err(V2LaneWorkError::RestartRequired);\n"
            "        }\n"
            "        self.lane_sessions\n"
            "            .insert_recovered_proposals(&raw_proposals)",
            "one complete bounded recovery batch before publishing payloads or historical READY",
            id="historical-capacity-replacement-before-count",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "record.historical_context_id,",
            "self.context.id(),",
            "one complete bounded recovery batch before publishing payloads or historical READY",
            id="historical-ready-context",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            ".saturating_add(recovered_historical_records.len())\n"
            "            > hydration_capacity",
            ".saturating_add(0)\n"
            "            > hydration_capacity",
            "historical and current autonomous payloads must share the exact bounded hydration inventory",
            id="historical-combined-payload-bound",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "let mut raw_proposals = Vec::new();",
            "let ordinary_hydration_capacity = hydration_capacity.saturating_sub(self.lane_sessions.len());\n"
            "        let mut raw_proposals = Vec::new();",
            "lane hydration must stage required proposals independently of retained cache occupancy",
            id="historical-retained-session-accounting",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "let mut recovered_historical_records = BTreeMap::new();",
            "let mut recovered_historical_records = self.historical_autonomous_recovery_records.clone();",
            "historical lane hydration must bound and authenticate every recovery record",
            id="historical-no-obsolete-map-resurrection",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            ".is_some_and(|existing| existing != &record)",
            ".is_some_and(|_| false)",
            "compare retained immutable identity before terminal skipping",
            id="historical-retained-immutable-record",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "let proposal = &record.payload.origin_proposal;\n"
            "            let key = AutonomousLanePayloadKey::from(proposal);",
            "let proposal = &record.payload.origin_proposal;\n"
            "            if self.kura.lane_block_application_receipt_available(proposal) { continue; }\n"
            "            let key = AutonomousLanePayloadKey::from(proposal);",
            "compare retained immutable identity before terminal skipping",
            id="historical-terminal-skip-before-immutable-check",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            ".validate_historical_autonomous_lane_recovery_record_dependencies(&record)",
            ".validate_historical_autonomous_lane_recovery_record_dependencies(&record).or(Ok(()))",
            "validate pending dependencies",
            id="historical-dependency-errors-propagate",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "raw_proposals.extend(\n            historical_ready_records\n                .iter()",
            "raw_proposals.extend(\n            self.historical_autonomous_recovery_records\n                .values()",
            "one complete bounded recovery batch before publishing payloads or historical READY",
            id="historical-no-obsolete-session-resurrection",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "self.historical_autonomous_recovery_records = recovered_historical_records.clone();\n"
            "        self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "publish the fresh historical inventory exactly once after complete batch installation",
            id="historical-no-early-map-publication",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "self.pending_autonomous_anchor_payloads = pending_autonomous_anchor_payloads.clone();\n"
            "        self.lane_sessions\n            .insert_recovered_proposals(&raw_proposals)",
            "publish pending payloads exactly once after complete batch installation",
            id="historical-no-early-payload-publication",
        ),
        pytest.param(
            "hydrate_canonical_lane_artifacts",
            "historical_ready_records.push(record);",
            "self.authorize_autonomous_ready_from_durable_input(&record.payload, proposal, record.historical_context_id)\n"
            "                .map_err(V2LaneWorkError::InvalidContext)?;\n"
            "            historical_ready_records.push(record);",
            "authorize historical READY exactly once after complete batch installation",
            id="historical-no-early-ready-authorization",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "if !raw_slots.insert((lane_id, lane_block_height)) {",
            "if false && !raw_slots.insert((lane_id, lane_block_height)) {",
            "raw lane hydration must fail stop on a duplicate or cyclic predecessor slot",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "raw_proposals.len().saturating_add(route_chain.len()) >= hydration_capacity",
            "raw_proposals.len().saturating_add(route_chain.len()) > hydration_capacity",
            "raw lane hydration must fail stop at the exact bounded inventory",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "read_lane_block_artifact_read_only(lane_id, lane_block_height)",
            "read_lane_block_artifact(lane_id, lane_block_height)",
            "read only the indexed immutable artifact",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "let artifact = self\n"
            "                    .consensus_storage_read(\n"
            "                        self.kura\n"
            "                            .read_lane_block_artifact_read_only(lane_id, lane_block_height),\n"
            "                    )?",
            "let artifact = self.kura\n"
            "                    .read_lane_block_artifact_read_only(lane_id, lane_block_height)?",
            "raw lane hydration must fail stop at the exact bounded inventory",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "read_lane_block_artifact_read_only(lane_id, lane_block_height),\n"
            "                    )?",
            "read_lane_block_artifact_read_only(lane_id, lane_block_height),\n"
            "                    ).ok().flatten()",
            "raw lane hydration must fail stop at the exact bounded inventory",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "|| !canonical_shape",
            "|| false",
            "raw lane hydration must reject malformed, inactive, or non-canonical carrier ownership",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "if canonical.as_slice() != [artifact.clone()] {",
            "if false {",
            "raw lane hydration must require one exact canonical ownership artifact",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "if !self.consensus_storage_read(canonical_raw_lane_predecessor_matches_proposal(",
            "if false && !self.consensus_storage_read(canonical_raw_lane_predecessor_matches_proposal(",
            "raw lane hydration must authenticate every unapplied predecessor link",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "route_chain.reverse();",
            "route_chain.clear();",
            "raw lane hydration must restore each predecessor chain in forward application order",
        ),
        (
            "hydrate_canonical_lane_artifacts",
            "raw_proposals.sort_by_key(|proposal| {\n"
            "            let descriptor = &proposal.descriptor;\n"
            "            (\n"
            "                descriptor.proposal_height,\n"
            "                descriptor.lane_id,\n"
            "                descriptor.dataspace_id,\n"
            "                descriptor.lane_block_height,\n"
            "                proposal.proposal_hash,\n"
            "            )\n"
            "        });",
            "raw_proposals.reverse();",
            "raw lane hydration must install independent chains in canonical deterministic order",
        ),
    ),
)
def test_lane_predecessor_ordering_mutations_survive_digest_refresh(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Refreshed lane item seals cannot hide raw/predecessor order weakening."""

    module = load_checker()
    relative = Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs")
    lane_path = tmp_path / relative
    lane_path.parent.mkdir(parents=True)
    shutil.copyfile(ROOT_DIR / relative, lane_path)
    context = (("impl", "V2LaneWorkAdapter"),)
    baseline_errors = _lane_predecessor_owner_contract_errors(module, lane_path)
    assert not baseline_errors, baseline_errors
    mutate_rust_item_source_in_context(
        module, lane_path, item_name, context, old, new
    )
    qualified = f"V2LaneWorkAdapter::{item_name}"
    if qualified in module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256:
        bindings = (
            (module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256, qualified),
        )
    else:
        assert item_name == "reconstruct_durable_lane_certificate"
        bindings = (
            (module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256, item_name),
        )
    original = rebind_reviewed_rust_item_digests(
        module, lane_path, item_name, context, bindings
    )
    try:
        errors = _lane_predecessor_owner_contract_errors(module, lane_path, item_name)
    finally:
        restore_reviewed_rust_item_digests(original)

    assert any(expected_error in error for error in errors), errors
    assert not any(
        item_name in error and "exact reviewed token digest" in error
        for error in errors
    ), errors


def _lane_predecessor_owner_contract_errors(
    module, lane_path: Path, refreshed_item: str | None = None,
) -> list[str]:
    """Check this exact owner; the full source-fidelity baseline runs separately."""

    source = lane_path.read_text(encoding="utf-8")
    errors = []
    lane_items = {}
    lane_ack_items = {}
    for name in (
        "proposal_predecessor_is_ready_for_progress",
        "persist_anchored_sessions",
        "reconstruct_durable_lane_certificate",
        "preflight_effect_insertion",
        "hydrate_canonical_lane_artifacts",
    ):
        item = module._require_qualified_rust_item(
            lane_path, source, "V2LaneWorkAdapter", name, errors,
            f"lane predecessor owner {name}",
        )
        qualified = f"V2LaneWorkAdapter::{name}"
        lane_items[name] = item
        lane_ack_items[qualified] = item
        if name == refreshed_item:
            digest = (
                module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256[qualified]
                if qualified in module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256
                else module._PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256[name]
            )
            module._require_rust_item_token_sha256(lane_path, item, digest, name, errors)
    module._require_lane_predecessor_ordering_source_contracts(
        lane_path, lane_ack_items, lane_items, errors,
    )
    return errors


@pytest.mark.parametrize(
    ("owner", "old", "new", "expected_error"),
    (
        (
            "worker",
            "if current_sources != self.source_fifo_owners\n"
            "            || current_reservations != self.reservation_owner_counts",
            "if current_sources == self.source_fifo_owners\n"
            "            || current_reservations != self.reservation_owner_counts",
            "worker cancellation must validate the complete pre-mutation FIFO and reservation projection",
        ),
        (
            "dispatch",
            "let _ = apply_retired_merge_sidecar_requests(lane_work, services)?;",
            "let _ = apply_retired_historical_recovery_requests(lane_work, services)?;",
            "runner lane dispatch must cancel retired source owners and close prefixes before admitting or dispatching later chunks",
        ),
        (
            "drain",
            "if retired == 0 && dispatched == 0 && after >= before {",
            "if retired == 0 && dispatched == 0 && after > before {",
            "durable finalization must cancel retired sources, apply receipts on both sides of handoff, drain dispatchable work, and reject a non-descending loop",
        ),
        (
            "schedule",
            "let _ = self.retire_inactive_merge_sidecar_requests(active_requests)?;",
            "let _ = active_requests;",
            "sidecar retransmission must cancel every retired transport attempt before handing off bounded successor posts",
        ),
        (
            "prune",
            "let _ = self.retire_inactive_merge_sidecar_requests(active_requests)?;",
            "let _ = active_requests;",
            "finalized sidecar pruning must retire exact requester output before Kura cleanup without fabricating a cursor receipt",
        ),
    ),
)
def test_extracted_exact_output_owner_mutations_survive_digest_refresh(
    tmp_path: Path, owner: str, old: str, new: str, expected_error: str
) -> None:
    """Extracted ownership helpers retain semantic checks after seal refresh."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    if owner == "worker":
        path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
        item_name = "remove_fanouts_matching"
        context = (("impl", "PendingExactOutput"),)
        bindings = ((module._PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256,
                     "PendingExactOutput::remove_fanouts_matching"),)
    elif owner in {"schedule", "prune"}:
        path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
        item_name = ("schedule_retransmission_at" if owner == "schedule" else
                     "prune_finalized_merge_sidecars")
        context = (("impl", "V2LaneWorkAdapter"),)
        bindings = ((module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256,
                     f"V2LaneWorkAdapter::{item_name}"),)
    else:
        item_name = ("dispatch_lane_work_effects_with_progress"
                     if owner == "dispatch" else "drain_finalized_lane_work_output")
        path = tmp_path / ("crates/iroha_core/src/sumeragi/v2_runner.rs"
                           if owner == "dispatch" else
                           "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs")
        context = ()
        bindings = (
            (module._PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256, item_name),
            (module._PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256, item_name),
        )
    mutate_rust_item_source_in_context(module, path, item_name, context, old, new)
    original = rebind_reviewed_rust_item_digests(
        module, path, item_name, context, bindings
    )
    try:
        errors = module._exact_output_production_source_fidelity_errors(tmp_path)
    finally:
        restore_reviewed_rust_item_digests(original)
    assert any(expected_error in error for error in errors), errors
    assert not any(item_name in error and "exact reviewed token digest" in error
                   for error in errors), errors


def test_applied_payload_and_recovered_fetch_refanout_mutations_fail_closed(
    tmp_path: Path,
) -> None:
    """Bind ticketless PayloadChunks and topology-rotated recovered Fetch retry."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    production_impl = (("impl", "ProductionV2Services"),)
    cfg_ticket_impl = (
        (
            "#", "[", "cfg", "(", "any", "(", "test", ",", "feature", "=",
            ")", ")", "]", "impl", "NetworkActorAdmissionTicketTestFixture",
        ),
    )
    mutations = (
        (
            "crates/iroha_core/src/sumeragi/v2_worker/autonomous_lane_output_reconstruction.rs",
            "applied_height_reconstruction_covers",
            (),
            "return payload_chunk_output_has_applied_height_authority(messages, manifest, artifact);",
            "return Ok(());",
        ),
        (
            "crates/iroha_core/src/sumeragi/tests/v2_worker_backpressure_retirement_cases.rs",
            "applied_height_finality_releases_only_covered_ticketless_payload_chunks",
            (),
            "assert!(\n        uncovered.is_pending(),",
            "assert!(\n        !uncovered.is_pending(),",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/queue_plan_admission_handoff.rs",
            "current_archive_targets_with_frozen_fallback",
            production_impl,
            "if !targets.is_empty() {\n            return targets;\n        }",
            "if false && !targets.is_empty() {\n            return targets;\n        }",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
            "recovered_decision_fetch_fanout",
            production_impl,
            "let peers = self.current_archive_targets_with_frozen_fallback(&owner.sources);",
            "let peers = owner.sources.clone();",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "cancel_topology_membership",
            cfg_ticket_impl,
            "membership.cancel();",
            "let _ = membership.is_active();",
        ),
        (
            "crates/iroha_core/src/sumeragi/tests/v2_worker_main_01.rs",
            "recovered_decision_fetch_refanout_reaches_live_peer_after_topology_ticket_cancellation",
            (),
            "configured_peers.replace(vec![rotated_peer.clone()]);",
            "configured_peers.replace(vec![frozen_source.clone()]);",
        ),
    )
    for relative, item_name, context, old, new in mutations:
        mutate_rust_item_source_in_context(
            module, tmp_path / relative, item_name, context, old, new
        )

    network_path = tmp_path / "crates/iroha_p2p/src/network.rs"
    mutate_source_once(
        network_path,
        '#[cfg(any(test, feature = "test-fixtures"))]\n#[derive(Debug)]\n'
        "pub struct ConfiguredPeerSnapshotTestFixture",
        '#[cfg(test)]\n#[derive(Debug)]\n'
        "pub struct ConfiguredPeerSnapshotTestFixture",
    )

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)
    expected_semantic_errors = (
        "ticketless PayloadChunks retirement must validate the exact fanout and scope",
        "applied-height authority must retain ticketless payload chunks from another creation scope",
        "historical archive target selection must prefer a non-empty live configured-peer snapshot",
        "recovered Decision Fetch refanout must preserve the signed WAL request",
        "cfg-gated cancellation capability must deactivate the exact topology tenure",
        "recovered Decision Fetch refanout test capability ConfiguredPeerSnapshotTestFixture",
        "recovered Fetch regression must rotate the configured archive",
    )
    for expected in expected_semantic_errors:
        assert any(
            expected in error and "exact reviewed token digest" not in error
            for error in errors
        ), (expected, errors)


def test_stable_liveness_repair_mutations_fail_closed(tmp_path: Path) -> None:
    """Reject weakened historical, recovery, advert, and planner ownership."""

    module = load_checker()
    relative_paths = (
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path(
            "crates/iroha_core/src/sumeragi/v2_lane_work/"
            "historical_recovery_and_carrier_tests.rs"
        ),
        Path(
            "crates/iroha_core/src/sumeragi/v2_lane_work/"
            "canonical_executed_block_application_repair.rs"
        ),
        Path(
            "crates/iroha_core/src/sumeragi/v2_runner/"
            "canonical_recovery_ingress.rs"
        ),
        Path("crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs"),
        Path(
            "crates/iroha_core/src/sumeragi/tests/"
            "v2_worker_backpressure_retirement_cases.rs"
        ),
        Path("crates/iroha_core/src/sumeragi/lane_planner.rs"),
        Path("crates/iroha_core/src/sumeragi/lane_planner_tests.rs"),
    )
    for relative in relative_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)

    check = module._stable_liveness_repairs_source_fidelity_errors
    assert check(tmp_path) == []
    lane_impl = (("impl", "V2LaneWorkAdapter"),)
    mutations = (
        (
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            "schedule_historical_recovery_request",
            lane_impl,
            "owner\n"
            "                    .canonical_body_destinations\n"
            "                    .extend(scheduled_destinations);",
            "owner.canonical_body_destinations.clear();",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
            "authenticates_certified_merge_sidecar_service_for_requester",
            lane_impl,
            "(requester_belongs_to(&self.context) || requester_belongs_to(historical_context))",
            "requester_belongs_to(historical_context)",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/v2_lane_work/"
                "canonical_executed_block_application_repair.rs"
            ),
            "service_next_with_archive_targets",
            (("impl", "CanonicalExecutedBlockRecovery"),),
            "let peer = outstanding.responder.peer.clone();",
            "let peer = self.local_peer.clone();",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs"),
            "drive_with_budget_ack_and_durable_history",
            (("impl", "PendingExactOutput"),),
            "ExactOutputRolloverClaim::DurableKuraReplicaAdvert { .. }\n"
            "                                            | ExactOutputRolloverClaim::QueuePlanAdmission { .. }",
            "ExactOutputRolloverClaim::DurableKuraReplicaAdvert { .. }",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs"),
            "drive_with_budget_ack_and_durable_history",
            (("impl", "PendingExactOutput"),),
            "released_kura_replica_advert_heights.insert(*source_height);",
            "let _ = source_height;",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_worker_backpressure_retirement_cases.rs"
            ),
            "terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission",
            (),
            "assert!(\n"
            "        ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"missing QueuePlan source retains ticketless output\"),",
            "assert!(\n"
            "        !ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"missing QueuePlan source retains ticketless output\"),",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_worker_backpressure_retirement_cases.rs"
            ),
            "terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission",
            (),
            "assert!(\n"
            "        ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"non-matching QueuePlan source retains ticketless output\"),",
            "assert!(\n"
            "        !ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"non-matching QueuePlan source retains ticketless output\"),",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_worker_backpressure_retirement_cases.rs"
            ),
            "terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission",
            (),
            "assert!(\n"
            "        !ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"terminal retry revalidates QueuePlan output from Kura\")\n"
            "    );",
            "assert!(\n"
            "        ticketless\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"terminal retry revalidates QueuePlan output from Kura\")\n"
            "    );",
        ),
        (
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "v2_worker_backpressure_retirement_cases.rs"
            ),
            "terminal_retry_revalidates_only_ticketless_exact_kura_queue_plan_admission",
            (),
            "assert!(\n"
            "        ticketed\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"live actor ticket retains QueuePlan output\")\n"
            "    );",
            "assert!(\n"
            "        !ticketed\n"
            "            .retry_pending_exact_output()\n"
            "            .expect(\"live actor ticket retains QueuePlan output\")\n"
            "    );",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/lane_planner.rs"),
            "is_retryable_after_state_or_kura_progress",
            (("impl", "AutonomousLaneReservationSlotPlanError"),),
            "Self::BlockedPredecessor { .. } | Self::PlanningSnapshotChanged",
            "Self::PlanningSnapshotChanged",
        ),
    )
    for relative, item_name, context, old, new in mutations:
        mutate_rust_item_source_in_context(
            module, tmp_path / relative, item_name, context, old, new
        )

    errors = check(tmp_path)
    expected_semantic_errors = (
        "every actually scheduled canonical-body archive must remain",
        "historical merge-sidecar service must accept an exact requester",
        "canonical executed-block retry must remain byte-identical and pinned",
        "ticketless Kura-backed retirement must revalidate only an exact advert or QueuePlan claim",
        "advert retirement must retain its durable source height",
        "QueuePlan output must remain owned when its exact Kura source is absent",
        "QueuePlan output must remain owned when Kura contains mismatched bytes",
        "ticketless QueuePlan output must release after State and exact Kura validation",
        "exact Kura history must not supersede QueuePlan output with a live actor ticket",
        "only predecessor blockage or a changed planning snapshot may retry",
    )
    for expected in expected_semantic_errors:
        assert any(
            expected in error and "exact reviewed token digest" not in error
            for error in errors
        ), (expected, errors)



def test_merge_frontier_deferral_current_owner_contract(tmp_path: Path) -> None:
    """The narrow typed-deferral owner passes independently of unrelated stale seals."""
    module = load_checker()
    lane_path = tmp_path / "v2_lane_work.rs"
    for name in ("v2_lane_work.rs", "v2_runner.rs"):
        shutil.copy2(ROOT_DIR / "crates/iroha_core/src/sumeragi" / name,
                     lane_path.with_name(name))
    errors: list[str] = []
    module._require_merge_frontier_deferral_contract(
        lane_path, lane_path.read_text(encoding="utf-8"), errors)
    assert not errors, errors


def test_merge_frontier_deferral_semantics_survive_digest_refresh(tmp_path: Path) -> None:
    """Resealing typed outcomes cannot authorize work or mask fatal stable errors."""
    module = load_checker()
    mutations = (
        ('merge_parent_frontier_at_generation', 'return Ok(MergeCandidateValidation::Deferred);', 'return Ok(MergeCandidateValidation::Ready);', 'odd or changing State generation'),
        ('merge_parent_frontier_at_generation', 'if durable_parent != Some(expected_parent) {', 'if false {', 'storage errors and stable contradictory frontier'),
        ('merge_parent_frontier_at_generation', '.map_err(|error| error.to_string())?;', '.unwrap_or(0);', 'storage errors and stable contradictory frontier'),
        ('merge_parent_frontier_at_generation', 'if durable_height_after != durable_height', 'if durable_height_after == durable_height', 'storage errors and stable contradictory frontier'),
        ('validate_merge_candidate_for_active_round', 'return Ok(MergeCandidateValidation::Deferred);', 'return Ok(MergeCandidateValidation::Ready);', 'each frontier deferral'),
        ('validate_merge_candidate_for_active_round', 'validation.map_err(|error| MergeCandidateValidationError::Invalid(error.to_string()))?;', 'let _ = validation;', 'stable live validation error'),
        ('defer_merge_candidate_work', 'self.purge_queued_merge_broadcasts();', 'let _ = &self.effects;', 'deferral purges only'),
        ('defer_merge_candidate_work', 'MergeRefreshOutcome::Deferred', 'MergeRefreshOutcome::Ready', 'deferral purges only'),
        ('authorize_local_merge_claim', 'let validation_generation = self.state.state_view_generation();\n        if self\n            .merge_parent_frontier_at_generation(validation_generation)\n            .map_err(MergeSidecarError::SigningGuard)?\n            == MergeCandidateValidation::Deferred\n        {\n            self.validated_merge_execution_candidate = None;\n            return Ok(LocalMergeAuthorization::Deferred);\n        }', 'let validation_generation = self.state.state_view_generation();\n        if self\n            .merge_parent_frontier_at_generation(validation_generation)\n            .map_err(MergeSidecarError::SigningGuard)?\n            == MergeCandidateValidation::Deferred\n        {\n            self.validated_merge_execution_candidate = None;\n            return Ok(LocalMergeAuthorization::Authorized);\n        }', 'signing validates the exact generation'),
        ('refresh_merge_candidates', 'Ok(LocalMergeAuthorization::Deferred) => {\n                    return Ok(self.defer_merge_candidate_work());\n                }', 'Ok(LocalMergeAuthorization::Deferred) => {}', 'signing and pending insertion'),
        ('accept_merge_signature', 'Ok(None) => {\n                    self.defer_merge_candidate_work();\n                    return Ok(V2LaneIngressOutcome::Rejected);\n                }', 'Ok(None) => return Ok(V2LaneIngressOutcome::Duplicate),', 'deferred leader validation'),
        ('schedule_retransmission_at', 'active_merge_view = None;', 'let _ = active_merge_view;', 'deferred refresh clears'),
        ('schedule_retransmission_at', 'self.schedule_merge_share_retransmissions(view)?;', 'let _ = view;', 'merge share retransmission'),
        ('schedule_retransmission_at', 'let Some(active_merge_view) = active_merge_view else {', 'let Some(active_merge_view) = self.pre_apply_unlocked_merge_view() else {', 'merge QC retry stops'),
        ('prepare', 'operation.complete();\n                return Err(all_unavailable(\n                    candidates.len(),\n                    "merge frontier is changing",\n                ));', 'drop(operation);\n                return Err(all_unavailable(candidates.len(), "merge frontier is changing"));', 'provider completes'),
        ('prepare_certified_execution_carrier', 'operation.complete();\n                return Err(all_unavailable(\n                    candidates.len(),\n                    "merge frontier is changing",\n                ));', 'drop(operation);\n                return Err(all_unavailable(candidates.len(), "merge frontier is changing"));', 'provider completes'),
        ('prepare', 'drop(operation);', 'operation.complete();', 'provider keeps refresh errors fatal'),
        ('defer_merge_frontier', 'CANDIDATE_WORK_RECHECK', 'NON_EMPTY_CANDIDATE_WORK_RECHECK', 'producer uses bounded recheck'),
        ('schedule_local_proposal', 'proposal_state.defer_merge_frontier(owner, Instant::now());', 'proposal_state.defer_candidate_work(owner, Instant::now());', 'producer returns before admitting'),
        ('refresh_merge_candidates', 'Err(MergeCandidateValidationError::Frontier(reason)) => {\n                    return Err(V2LaneWorkError::SigningGuard(reason));\n                }', 'Err(MergeCandidateValidationError::Frontier(_)) => {}', 'installed candidates propagate'),
        ('accept_merge_signature', 'Err(MergeCandidateValidationError::Frontier(reason)) => {\n                    return Err(V2LaneWorkError::SigningGuard(reason));\n                }', 'Err(MergeCandidateValidationError::Frontier(_)) => return Ok(V2LaneIngressOutcome::Rejected),', 'leader filtering never suppresses'),
        ('refresh_merge_candidates', 'let parent_header = self.state.latest_block_header_fast();', 'let parent_header = None;', 'parent snapshot recheck'),
        ('defer_merge_candidate_work', 'self.purge_queued_merge_broadcasts();', 'self.purge_queued_merge_broadcasts(); self.merge_claims.clear();', 'deferral purges only'),
        ('new_with_output_guard_and_transport_inner', '#[cfg(test)]\n            merge_validation_test_hook: None,', 'merge_validation_test_hook: None,', 'constructor frontier hook remains test-only'),
        ('authorize_local_merge_claim', 'if candidate.view != active_view', 'if false', 'intrinsic candidate mismatch is fatal'),
        ('authorize_local_merge_claim', 'if self.pre_apply_unlocked_merge_view() != Some(active_view) {\n            self.validated_merge_execution_candidate = None;\n            return Ok(LocalMergeAuthorization::Deferred);\n        }', 'if self.pre_apply_unlocked_merge_view() != Some(active_view) { return Ok(LocalMergeAuthorization::Authorized); }', 'intrinsic candidate mismatch is fatal'),
        ('schedule_retransmission_at', 'self.collect_committed_lane_sessions()?;', 'let _ = self.collect_committed_lane_sessions();', 'scheduler preserves fatal collect_committed_lane_sessions'),
        ('schedule_retransmission_at', 'self.purge_queued_global_body_effects_except_committed_outputs()?;', 'let _ = self.purge_queued_global_body_effects_except_committed_outputs();', 'scheduler preserves fatal purge_queued_global_body_effects_except_committed_outputs'),
        ('schedule_retransmission_at', 'self.schedule_committed_lane_outputs()?;', 'let _ = self.schedule_committed_lane_outputs();', 'scheduler preserves fatal schedule_committed_lane_outputs'),
    )
    for index, (name, old, new, diagnostic) in enumerate(mutations):
        fixture_root = tmp_path / str(index)
        fixture_root.mkdir()
        lane_path = fixture_root / "v2_lane_work.rs"
        for filename in ("v2_lane_work.rs", "v2_runner.rs"):
            shutil.copy2(ROOT_DIR / "crates/iroha_core/src/sumeragi" / filename,
                         fixture_root / filename)
        baseline: list[str] = []
        module._require_merge_frontier_deferral_contract(
            lane_path, lane_path.read_text(encoding="utf-8"), baseline)
        assert not baseline, baseline
        relative, context, _attributes, seal_map, seal_key = module._MERGE_FRONTIER_DEFERRAL_OWNERS[name]
        path = fixture_root / Path(relative).name
        source = path.read_text(encoding="utf-8")
        original = next(item for item in module.rust_items(source, name)
                        if item.brace_context == context)
        assert old in original.source, (name, old)
        # Replace one deliberate semantic branch; other occurrences stay unchanged.
        changed = original.source.replace(old, new, 1)
        assert changed != original.source
        path.write_text(source.replace(original.source, changed, 1), encoding="utf-8")
        changed_item = next(item for item in module.rust_items(path.read_text(encoding="utf-8"), name)
                            if item.brace_context == context)
        seals = getattr(module, seal_map)
        old_seal = seals[seal_key]
        try:
            seals[seal_key] = module._rust_item_token_sha256(changed_item)
            errors: list[str] = []
            module._require_merge_frontier_deferral_contract(
                lane_path, lane_path.read_text(encoding="utf-8"), errors)
            assert any(diagnostic in error and "exact reviewed token digest" not in error
                       for error in errors), errors
            assert not any("exact reviewed token digest" in error for error in errors), errors
        finally:
            seals[seal_key] = old_seal
