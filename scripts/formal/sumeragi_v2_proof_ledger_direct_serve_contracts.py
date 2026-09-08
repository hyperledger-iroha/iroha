"""Coordinator-owned Certified-Serve production source-fidelity contracts."""


_WORKER_OWNERSHIP_RECONCILIATION_OWNERS = {
    "applied_height_reconstruction_covers": ("v2_worker/autonomous_lane_output_reconstruction.rs", "applied_height_reconstruction_covers", ""),
    "validate_fanout_bounds": ("v2_worker_exact_output.rs", "validate_fanout_bounds", "impl PendingExactOutput"),
    "start": ("v2_worker_services_impl.rs", "start", "impl ProductionV2Services"),
    "start_inner": ("v2_worker_services_impl.rs", "start_inner", "impl ProductionV2Services"),
    "durable_history_source_covers": ("v2_worker_exact_output.rs", "durable_history_source_covers", ""),
    "validate_fanout": ("v2_worker/exact_output_rollover_claim.rs", "validate_fanout", "impl ExactOutputRolloverClaim"),
    "post_durable_history_response_with_routes": ("v2_worker_services_impl.rs", "post_durable_history_response_with_routes", "impl ProductionV2Services"),
    "proof_network_id": ("v2_historical_body_serve.rs", "network_id", "impl HistoricalBodyDurableSourceProof"),
    "proof_source_round": ("v2_historical_body_serve.rs", "source_round", "impl HistoricalBodyDurableSourceProof"),
    "proof_layout": ("v2_historical_body_serve.rs", "HistoricalBodyDurableSourceProof", ""),
    "proof_covers_message": ("v2_historical_body_serve.rs", "covers_message_in_network", "impl HistoricalBodyDurableSourceProof"),
    "network_exact_output_hash": ("message.rs", "exact_output_hash", "impl crate::NetworkMessage"),
}

_WORKER_OWNERSHIP_RECONCILIATION_EXISTING_SEALS = {
    "applied_height_reconstruction_covers": (("_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256", "applied_height_reconstruction_covers"),),
    "validate_fanout_bounds": (("_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256", "PendingExactOutput::validate_fanout_bounds"),),
    "start": (("_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256", "ProductionV2Services::start"),),
    "start_inner": (("_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256", "ProductionV2Services::start_inner"),),
    "durable_history_source_covers": (("_PRODUCTION_DURABLE_HISTORY_WORKER_ITEM_SHA256", "durable_history_source_covers"),),
    "validate_fanout": (("_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256", "validate_fanout"),),
    "post_durable_history_response_with_routes": (("_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256", "post_durable_history_response_with_routes"),),
    "proof_covers_message": (("_PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256", "proof_covers_message"),),
    "network_exact_output_hash": (("_PRODUCTION_WORKER_ACK_RECONCILIATION_ITEM_SHA256", "network_exact_output_hash"),),
}


def _require_prepared_body_rollover_source_contracts(
    path: Path, item: RustItem | None, errors: list[str],
) -> None:
    """Consume only the exact opaque worker proof in the applied network/height."""

    _require_rust_token_sequence(path, item, """
ExactOutputRolloverClaim::DurableCertifiedBodyResponse { network_id, proof, .. },
_,
) => {
    if proof.source_round().height > maximum_source_height {
        return Err("durable body response belongs to a future height".to_owned());
    }
    if network_id != source_network_id
        || proof.network_id() != *source_network_id
        || !proof.covers_message_in_network(source_network_id, message)
    {
        return Err("durable body response differs from its prepared Kura source".to_owned(),);
    }
    Ok(())
}
""", "durable body response must match its exact canonical Kura block through the prepared proof", errors)


def _worker_ownership_reconciled_source_fidelity_errors(repo_root: Path = ROOT_DIR) -> list[str]:
    """Bind worker creation and exact opaque-proof admission/retirement owners."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    items: dict[str, tuple[Path, RustItem | None]] = {}
    auxiliary = set(_WORKER_OWNERSHIP_RECONCILIATION_OWNERS) - set(_WORKER_OWNERSHIP_RECONCILIATION_EXISTING_SEALS)
    if set(_PRODUCTION_WORKER_OWNERSHIP_RECONCILIATION_ITEM_SHA256) != auxiliary:
        errors.append("worker ownership auxiliary seals must equal the reviewed owner inventory")
    for key, (relative, name, context) in _WORKER_OWNERSHIP_RECONCILIATION_OWNERS.items():
        if relative not in sources:
            sources[relative] = _read_reviewed_rust_source(repo_root,
                f"crates/iroha_core/src/sumeragi/{relative}", errors, f"worker ownership {relative}")
        path, source = sources[relative]
        candidates = rust_struct_items(source, name) if key == "proof_layout" else _ingress_effects_parsed_items(source, name)
        matches = tuple(item for item in candidates
            if item.brace_context == ((rust_code_tokens(context),) if context else ()))
        item = matches[0] if len(matches) == 1 else None
        if item is None:
            errors.append(f"{path}: worker ownership requires exactly one {context}::{name} owner")
        attributes = ("#[allow(clippy::too_many_arguments, dead_code)]",) if key == "start" else (
            ("#[allow(clippy::too_many_arguments)]",) if key == "start_inner" else (
            ("#[derive(Clone, Debug, PartialEq, Eq)]",) if key == "proof_layout" else ()))
        _require_rust_item_context(path, item, (rust_code_tokens(context),) if context else (),
            key, errors, expected_attributes=attributes)
        for mapping_name, seal_key in _WORKER_OWNERSHIP_RECONCILIATION_EXISTING_SEALS.get(key,
            (("_PRODUCTION_WORKER_OWNERSHIP_RECONCILIATION_ITEM_SHA256", key),)):
            mapping = globals().get(mapping_name, {})
            if seal_key not in mapping:
                errors.append(f"{path}: worker ownership {key} is missing its {mapping_name} source seal")
            else:
                _require_rust_item_token_sha256(path, item, mapping[seal_key], key, errors)
        items[key] = (path, item)

    def require(key: str, sequence: str, label: str, count: int = 1) -> None:
        _require_rust_token_sequence(*items[key], sequence, label, errors, count=count)

    require("proof_layout", """
pub(crate) struct HistoricalBodyDurableSourceProof {
    network_id: NetworkId, source_round: wire::ConsensusRound,
    source_subject: wire::BlockSubject, responder: PeerId,
    request_hash: HashOf<wire::CertifiedBodyRequest>, exact_output_hash: HashOf<NetworkMessage>,
}
""", "prepared body proof exposes no mutable or forgeable source identity fields")
    require("proof_network_id", "pub(crate) const fn network_id(&self) -> NetworkId { self.network_id }",
        "prepared body network accessor returns its immutable minted network")
    require("proof_source_round", "pub(crate) const fn source_round(&self) -> wire::ConsensusRound { self.source_round }",
        "prepared body round accessor returns its immutable minted height and context")
    require("proof_covers_message", """
if &self.network_id != expected_network_id { return false; }
let NetworkMessage::SumeragiBlock(envelope) = message else { return false; };
let BlockMessage::V2(v2) = envelope.as_message() else { return false; };
let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &v2.payload else { return false; };
response.request_hash == self.request_hash
    && response.manifest.round == self.source_round
    && response.manifest.subject == self.source_subject
    && response.responder == self.responder
    && message.cached_exact_output_hash() == Some(self.exact_output_hash)
""", "prepared proof authenticates exact network, response family, request, source, responder and warmed whole-message hash")
    require("network_exact_output_hash", """
match self {
    Self::SumeragiBlock(envelope) => { *envelope.exact_output_hash.get_or_init(|| HashOf::new(self)) }
    _ => HashOf::new(self),
}
""", "worker bounds use the canonical whole-message cached identity")
    _require_prepared_body_rollover_source_contracts(*items["durable_history_source_covers"], errors)
    require("durable_history_source_covers", """
let [message] = messages else {
    return Err("Sumeragi v2 durable response claim is not a singleton".to_owned());
};
if message.progress_reconstruction() != ProgressReconstruction::Retransmit {
    return Err("Sumeragi v2 durable response is not reconstructible traffic".to_owned());
}
let NetworkMessage::SumeragiBlock(envelope) = message else {
    return Err("Sumeragi v2 durable response is not block traffic".to_owned());
};
""", "durable proof consumption first rejects non-singleton, exact-only or foreign traffic")
    require("validate_fanout", """
Self::DurableCertifiedBodyResponse { target, network_id, proof, .. } => {
    let [message] = messages else {
        return Err("durable body response claim requires one exact message".to_owned());
    };
    if peers != std::slice::from_ref(target)
        || proof.network_id() != *network_id
        || !proof.covers_message_in_network(network_id, message)
    {
        return Err("durable body response claim changed identity".to_owned());
    }
    Ok(())
}
""", "prepared body fanout retains one exact message, target, network and authenticated proof")
    require("post_durable_history_response_with_routes", """
wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_) => {
    return Err("historical body output must cross the bounded prepared-worker seam".to_owned(),);
}
""", "synchronous historical posting rejects body responses before encoding or admission")
    require("post_durable_history_response_with_routes", """
wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)
    if response.certificate.round.height <= self.context.height && response.responder == self.local_peer =>
{
    ExactOutputRolloverClaim::DurableCommitCertificateResponse {
        scope: self.exact_output_scope(), target: peer.clone(), responder: self.local_peer.clone(),
        source_height: response.certificate.round.height,
        source_context_id: response.certificate.round.context_id,
        response_hash: HashOf::new(response),
    }
}
""", "synchronous CommitQC response keeps local responder, exact hash and non-future creation scope")
    require("post_durable_history_response_with_routes", """
rollover_claim.validate_fanout(&messages, &peers)?;
durable_history_source_covers(
    &messages, &rollover_claim, &self.context.network_id, self.context.height, self.kura.as_ref(),
)?;
let ownership = match reply_routes {
""", "synchronous CommitQC posting validates exact fanout and Kura before transferring output ownership")
    require("applied_height_reconstruction_covers", """
rollover_claim.validate_fanout(messages, peers)?;
if matches!(rollover_claim, ExactOutputRolloverClaim::NonRetireableLaneTransport { .. }) {
    return Err("non-retireable lane transport must drain before applied-height handoff".to_owned(),);
}
let scope = rollover_claim.scope().ok_or_else(|| {
    "Sumeragi v2 exact output has no typed applied-height rollover claim".to_owned()
})?;
if !scope.covers(artifact) {
    return Err("Sumeragi v2 output claim belongs to another creation scope".to_owned());
}
""", "applied retirement validates typed fanout and exact creation scope before any role shortcut")
    require("applied_height_reconstruction_covers", """
return durable_history_source_covers(
    messages, rollover_claim, &artifact.height_context.network_id, artifact.height,
    durable_history.ok_or_else(|| {
        "Sumeragi v2 durable response lacks an independently readable history source".to_owned()
    })?,
);
""", "applied retirement carries the artifact network and height into prepared-proof validation")
    require("applied_height_reconstruction_covers", "round.context_id == context_id && round.height == height",
        "ordinary global retirement still binds both context and height")
    require("applied_height_reconstruction_covers", "wire::ConsensusMessageV2Payload::PayloadManifest",
        "retired standalone manifest cannot reenter the global rollover role table", count=0)
    require("applied_height_reconstruction_covers", """
wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
    round_matches(certificate.round)
}
wire::ConsensusMessageV2Payload::PayloadChunk(_) => false,
wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
    round_matches(request.round)
}
""", "global role table preserves timeout and request authority while excluding unclaimed chunks")
    require("validate_fanout_bounds", """
if fanout.message_hashes.len() != fanout.messages.len()
    || fanout.message_classes.len() != fanout.messages.len()
    || fanout.message_class_suffixes.len().checked_sub(1) != Some(fanout.messages.len())
{
    return Err("Sumeragi v2 outbound fanout lost its immutable message index".to_owned());
}
if fanout.messages.iter().zip(&fanout.message_hashes).zip(&fanout.message_classes)
    .any(|((message, expected_hash), expected_class)| {
        message.exact_output_hash() != *expected_hash
            || exact_output_class(message).as_ref() != Ok(expected_class)
    })
{
    return Err("Sumeragi v2 outbound fanout changed its immutable messages".to_owned());
}
""", "worker admission checks complete immutable indexes, canonical cached hashes and traffic classes")
    require("validate_fanout_bounds", """
if fanout.current_source_targets != fanout.expected_current_source_targets()? {
    return Err("Sumeragi v2 outbound fanout changed its local FIFO index".to_owned());
}
let _ = fanout.outstanding_sources()?;
Ok(())
""", "worker admission validates every future source and FIFO before capacity may report backpressure")
    require("start", """
Self::start_inner(
    context, initial_tag, durable_decided_subject, validator_set_pops, local_peer, local_validator,
    None, key_pair, network, body_store, None, state, kura, apply_service,
    consensus_io_capacity, auxiliary_io_capacity, orphan_chunk_capacity, output_guard,
    leader_wire_ingress, kura_replica_advert_refresh, leader_wire_recovery_authority,
    exact_output_handoff_owner,
)
""", "ordinary worker constructor passes exact authorities and capacities without fabricating lifecycle authority")
    require("start_inner", """
let construction_guard = Arc::clone(&output_guard);
let construction = construction_guard.begin_fail_stop_operation()
    .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
if consensus_io_capacity == 0 || auxiliary_io_capacity == 0 || orphan_chunk_capacity == 0 {
    return Err("Sumeragi v2 service queue capacities must be non-zero".to_owned());
}
if initial_tag.height() != context.height {
    return Err("Sumeragi v2 service tag is outside its immutable height context".to_owned(),);
}
""", "worker construction holds fail-stop authority and validates capacity and tag before side effects")
    require("start_inner", """
let shared_pending_ownership_unit_capacity = sumeragi_v2_exact_output_shared_ownership_capacity(
    consensus_io_capacity, auxiliary_io_capacity,
).map_err(|error| error.to_string())?;
validate_shared_ownership_geometry(shared_pending_ownership_unit_capacity, reply_route_source_capacity,)?;
let frozen_semantic_targets = context.roster.iter().map(|entry| entry.validator.clone()).collect::<Vec<_>>();
let pending_exact_output = PendingExactOutput::new(
    shared_pending_ownership_unit_capacity, max_messages_per_fanout, max_peers_per_fanout, &frozen_semantic_targets,
)?;
""", "worker construction validates configured exact-output geometry against the complete frozen roster")
    require("start_inner", """
let lifecycle_body_store_identity = body_store.instance_identity();
let io = V2IoHandle::spawn(
    body_store, apply_service, context.clone(), key_pair.clone(), local_validator,
    kagemusha_mint_finality_authority, auxiliary_io_capacity, consensus_io_capacity,
    reply_route_source_capacity, Arc::clone(&output_guard),
)?;
""", "worker spawn transfers the real body store, Apply service and explicit finality authority under its guard")
    require("start_inner", """
clean_teardown: true,
};
construction.complete();
service.clean_teardown = false;
Ok(service)
""", "worker constructor completes abnormal-exit ownership only after the full service exists")
    for key in ("start", "start_inner"):
        for forbidden in ("chunk_root", "context_chunk_root", "create_dir_all"):
            require(key, forbidden, "worker construction cannot restore unused raw chunk-directory ownership", count=0)
    # Guard/geometry checks must precede the spawn, even if a mutant keeps every
    # required clause by moving a complete block after that side effect.
    path, item = items["start_inner"]
    if item is not None:
        tokens = rust_code_tokens(item.body)
        positions = [tokens.index(name) if name in tokens else -1 for name in (
            "begin_fail_stop_operation", "validate_shared_ownership_geometry", "PendingExactOutput", "V2IoHandle", "complete")]
        if -1 in positions or positions != sorted(positions):
            errors.append(f"{path}:{item.line}: worker creation validates ownership geometry before spawning and completes only afterward")
    return errors


_WORKER_ACK_RECONCILIATION_OWNERS = {
    "classified_with_route_history": ("v2_worker_io_execution.rs", "classified_with_route_history", "impl PendingExactFanout"),
    "plan_fanout_removal": ("v2_worker_exact_output.rs", "plan_fanout_removal", "impl PendingExactOutput"),
    "handoff_applied_height_to_durable_reconstruction": ("v2_worker_exact_output.rs", "handoff_applied_height_to_durable_reconstruction", "impl PendingExactOutput"),
    "enqueue_owned_exact_reply_routes_while_guarded": ("v2_worker_services_impl.rs", "enqueue_owned_exact_reply_routes_while_guarded", "impl ProductionV2Services"),
    "finish_height": ("v2_worker_services_impl.rs", "finish_height", "impl ProductionV2Services"),
    "retain_returned": ("v2_worker_io_execution.rs", "retain_returned", "impl PendingExactFanout"),
    "enqueue_exact_fanout_while_guarded_collecting_released_adverts": ("v2_worker_services_impl.rs", "enqueue_exact_fanout_while_guarded_collecting_released_adverts", "impl ProductionV2Services"),
    "network_exact_output_hash": ("message.rs", "exact_output_hash", "impl crate::NetworkMessage"),
    "wire_make_mut": ("message.rs", "make_mut", "impl BlockMessageWire"),
    "body_retirement_job": ("v2_body_store.rs", "into_retirement_job", "impl V2BodyStore"),
    "body_retirement_execute": ("v2_body_store.rs", "execute", "impl V2BodyRetirementJob"),
    "cleanup_execute": ("v2_worker_completion.rs", "execute_post_finality_cleanup", ""),
}

_WORKER_ACK_RECONCILIATION_EXISTING_SEALS = {
    "classified_with_route_history": (("_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256", "PendingExactFanout::classified_with_route_history"),),
    "plan_fanout_removal": (("_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256", "PendingExactOutput::plan_fanout_removal"),),
    "handoff_applied_height_to_durable_reconstruction": (
        ("_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256", "PendingExactOutput::handoff_applied_height_to_durable_reconstruction"),
        ("_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256", "handoff_applied_height_to_durable_reconstruction"),
        ("_REPLY_WRITER_DEADLINE_WORKER_ITEM_SHA256", "PendingExactOutput::handoff_applied_height_to_durable_reconstruction"),
    ),
    "enqueue_owned_exact_reply_routes_while_guarded": (
        ("_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256", "ProductionV2Services::enqueue_owned_exact_reply_routes_while_guarded"),
        ("_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256", "enqueue_owned_exact_reply_routes_while_guarded"),
    ),
    "finish_height": (("_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256", "ProductionV2Services::finish_height"),),
    "retain_returned": (("_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256", "retain_returned"),),
    "enqueue_exact_fanout_while_guarded_collecting_released_adverts": (
        ("_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256", "enqueue_exact_fanout_while_guarded_collecting_released_adverts"),
        ("_STABLE_LIVENESS_REPAIR_ITEM_SHA256", "ProductionV2Services::enqueue_exact_fanout_while_guarded_collecting_released_adverts"),
    ),
}


def _worker_ack_reconciled_source_fidelity_errors(repo_root: Path = ROOT_DIR) -> list[str]:
    """Bind cached immutable output identity and post-lock/receipt-owned cleanup."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    items: dict[str, tuple[Path, RustItem | None]] = {}
    auxiliary = set(_WORKER_ACK_RECONCILIATION_OWNERS) - set(_WORKER_ACK_RECONCILIATION_EXISTING_SEALS)
    if set(_PRODUCTION_WORKER_ACK_RECONCILIATION_ITEM_SHA256) != auxiliary:
        errors.append("worker ACK auxiliary seals must equal the reviewed owner inventory")
    for key, (relative, name, context) in _WORKER_ACK_RECONCILIATION_OWNERS.items():
        if relative not in sources:
            sources[relative] = _read_reviewed_rust_source(repo_root,
                f"crates/iroha_core/src/sumeragi/{relative}", errors, f"worker ACK {relative}")
        path, source = sources[relative]
        matches = tuple(item for item in _ingress_effects_parsed_items(source, name)
            if item.brace_context == ((rust_code_tokens(context),) if context else ()))
        item = matches[0] if len(matches) == 1 else None
        if item is None:
            errors.append(f"{path}: worker ACK requires exactly one {context}::{name} owner")
        _require_rust_item_context(path, item, (rust_code_tokens(context),) if context else (),
            key, errors, expected_attributes=())
        seals = _WORKER_ACK_RECONCILIATION_EXISTING_SEALS.get(key,
            (("_PRODUCTION_WORKER_ACK_RECONCILIATION_ITEM_SHA256", key),))
        for mapping_name, seal_key in seals:
            mapping = globals().get(mapping_name, {})
            if seal_key not in mapping:
                errors.append(f"{path}: worker ACK {key} is missing its {mapping_name} source seal")
            else:
                _require_rust_item_token_sha256(path, item, mapping[seal_key], key, errors)
        items[key] = (path, item)

    def require(key: str, sequence: str, label: str, count: int = 1) -> None:
        _require_rust_token_sequence(*items[key], sequence, label, errors, count=count)

    require("network_exact_output_hash", """
match self {
    Self::SumeragiBlock(envelope) => {
        *envelope.exact_output_hash.get_or_init(|| HashOf::new(self))
    }
    _ => HashOf::new(self),
}
""", "cached network output identity is exactly the canonical whole-message hash for every variant")
    require("wire_make_mut", """
self.encoded = None;
let _ = self.exact_output_hash.take();
Arc::make_mut(&mut self.message)
""", "mutable wire access invalidates both bytes and exact identity before exposing the message")
    require("classified_with_route_history", """
let message_hashes = messages.iter().map(NetworkMessage::exact_output_hash).collect();
""", "fanout classification pins every immutable message through the canonical cached hash")
    require("classified_with_route_history", """
let mut fanout = Self {
    messages, message_hashes, message_classes, message_class_suffixes, peers, targets,
    reply_routes, ingress_ownership: None, current_source_targets: BTreeMap::new(),
    next_target_index: 0, fifo_id: None, rollover_claim: ExactOutputRolloverClaim::Exact,
};
fanout.rebuild_current_source_targets()?;
Ok(Some(fanout))
""", "classified fanout retains its full route geometry and rebuilds exact source ownership")
    require("plan_fanout_removal", """
if fanout.message_hashes.len() != fanout.messages.len()
    || fanout.messages.iter().zip(&fanout.message_hashes)
        .any(|(message, expected)| message.exact_output_hash() != *expected)
{
    return Err(format!("Sumeragi v2 {operation} found altered exact-output payload"));
}
""", "removal planning authenticates every cached payload identity before selecting covered fanouts")
    require("plan_fanout_removal", """
if covered(fanout) {
    validate_removed(fanout)?;
    if !removed_fifo_ids.insert(fifo_id) {
        return Err(format!("Sumeragi v2 {operation} found duplicate exact-output FIFO ownership"));
    }
    continue;
}
""", "removal planning validates each selected FIFO owner before excluding it from retained accounting")
    require("plan_fanout_removal", """
if current_sources != self.source_fifo_owners || current_reservations != self.reservation_owner_counts {
    return Err(format!("Sumeragi v2 {operation} found inconsistent exact-output ownership"));
}
""", "removal planning rejects inconsistent source and reservation indexes before publishing its plan")
    require("handoff_applied_height_to_durable_reconstruction", """
if fanout.message_hashes.len() != fanout.messages.len()
    || fanout.messages.iter().zip(&fanout.message_hashes)
        .any(|(message, expected_hash)| message.exact_output_hash() != *expected_hash)
{
    return Err("Sumeragi v2 retained output changed before finality handoff".to_owned(),);
}
""", "applied-height handoff must preflight every pinned payload before classification")
    require("handoff_applied_height_to_durable_reconstruction", """
applied_height_reconstruction_covers(
    &fanout.messages, &fanout.semantic_peers(), &fanout.rollover_claim,
    artifact, durable_lane_authority, durable_history,
)?;
""", "handoff requires exact finality, semantic targets and durable reconstruction authority")
    require("handoff_applied_height_to_durable_reconstruction", """
if current.data.exact_output_hash() != *expected_hash {
    return Err("Sumeragi v2 returned output changed before finality handoff".to_owned(),);
}
""", "handoff validates each returned actor post against the pinned canonical payload")
    require("handoff_applied_height_to_durable_reconstruction", """
if self.ownership_units != expected_ownership_units
    || self.shared_ownership_units != expected_shared_ownership_units
{
    return Err("Sumeragi v2 outbound ownership totals changed before finality handoff".to_owned(),);
}
let sidecar_completions = self.admitted_sidecar_chunks.len();
remaining_posts = remaining_posts.checked_add(sidecar_completions)
    .ok_or_else(|| "Sumeragi v2 applied-height output count overflowed".to_owned())?;
self.fanouts.clear();
self.admitted_sidecar_chunks.clear();
self.next_fanout_index = 0;
self.next_fanout_fifo_id = 0;
self.source_fifo_owners.clear();
self.reservation_owner_counts.clear();
self.ownership_units = 0;
self.shared_ownership_units = 0;
Ok(remaining_posts)
""", "handoff preflights ownership totals and checked sidecar count before atomically clearing the corridor")
    require("retain_returned", """
if target.parked {
    return Err("Sumeragi v2 returned output to a parked reply source".to_owned());
}
if target.pending_flush.is_some() {
    return Err("Sumeragi v2 returned output over a pending writer flush".to_owned());
}
let expected_hash = self.message_hashes.get(target.message_index).ok_or_else(|| {
    "Sumeragi v2 exact-output target has no expected payload identity".to_owned()
})?;
""", "returned actor ownership rejects parked or flushing sources before selecting its exact payload index")
    require("retain_returned", """
if post.data.exact_output_hash() != *expected_hash {
    return Err("Sumeragi v2 network actor changed an exact output payload".to_owned());
}
debug_assert!(target.current.is_none());
debug_assert!(target.ticket.is_none());
target.current = Some(post);
target.ticket = ticket;
""", "returned actor post must retain the exact pinned payload identity")
    require("enqueue_owned_exact_reply_routes_while_guarded", """
if reply_routes.semantic_target() != &peer {
    return Err("Sumeragi v2 reply route does not match its semantic output target".to_owned(),);
}
let Some(fanout) = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
    vec![message], peer, reply_routes, ingress_ownership, rollover_claim,
)?
""", "guarded reply enqueue retains the exact semantic target, source routes and ingress claim")
    require("enqueue_owned_exact_reply_routes_while_guarded", """
let mut released_kura_replica_advert_heights = BTreeSet::new();
let ownership = {
    let mut pending = self.lock_pending_exact_output()?;
    if self.exact_output_handoff_owner.is_sealed() {
        return Err("Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),);
    }
    let ownership = pending.enqueue_owned_reply_transfer(fanout)?;
    if ownership == ExactFanoutOwnership::Owned {
        self.drive_pending_exact_output(&mut pending, &mut released_kura_replica_advert_heights,)
            .map(|_| ownership)
    } else {
        Ok(ownership)
    }
};
self.schedule_released_kura_replica_advert_heights(released_kura_replica_advert_heights)?;
ownership
""", "reply enqueue releases the corridor lock and schedules every released advert before returning even a failed drive result")
    require("enqueue_exact_fanout_while_guarded_collecting_released_adverts", """
let Some(fanout) = PendingExactFanout::claimed(messages, peers, rollover_claim)? else {
    return Ok(ExactFanoutOwnership::Owned);
};
{
    let mut pending = self.lock_pending_exact_output()?;
    if self.exact_output_handoff_owner.is_sealed() {
        return Err("Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),);
    }
    let ownership = pending.enqueue(fanout)?;
""", "every production exact fanout must enter the corridor with its typed claim")
    require("enqueue_exact_fanout_while_guarded_collecting_released_adverts", """
self.drive_pending_exact_output(&mut pending, released_kura_replica_advert_heights)
    .map(|_| ownership)
""", "guarded fanout returns the drive result while preserving released adverts for its post-lock caller")
    require("finish_height", """
let incomplete_exact_output_handoff = match self.pending_exact_output.lock() {
    Ok(_) if !self.exact_output_handoff_owner.is_sealed() => {
        Some("durable exact-output handoff was not sealed before finalized cleanup")
    }
    Ok(pending) if pending.is_pending() => {
        Some("durable exact-output handoff was sealed with pending output")
    }
    Ok(_) => None,
    Err(_) => {
        Some("durable exact-output corridor lock was poisoned before finalized cleanup")
    }
};
if let Some(reason) = incomplete_exact_output_handoff {
    outcome.record(PostFinalityCleanupTarget::CleanupWorker, reason);
    self.output_guard.activate_restart_required();
} else {
    self.clean_teardown = true;
}
""", "finalized cleanup requires a sealed empty corridor or activates restart before retirement")
    require("finish_height", """
self.retire_held_io_completion();
if let Some(mut io) = self.io.take() {
    let mut command = V2IoCommand::Retire(V2RetireCommand {
        receipt, cleanup: supervisor.submission(),
    });
""", "finalized cleanup transfers the actual receipt and bounded janitor owner without a raw directory path")
    require("finish_height", """
let enqueue = io.try_enqueue(command);
drop(retirement_enqueue_permit);
match enqueue {
    Ok(()) => break,
    Err(V2IoTrySendError::Full(returned)) => {
        command = returned;
        match recv_cleanup_completion(&io, deadline) {
""", "finalized cleanup preserves a full returned retirement command and drops its permit before waiting")
    require("finish_height", """
io.allow_finalized_disconnect.store(true, AtomicOrdering::Release);
break 'enqueue;
""", "only the configured cleanup deadline authorizes finalized disconnect before dropping the sender")
    require("body_retirement_job", """
self.ensure_mutable()?;
if kura_receipt.context_id() != self.context.id() || kura_receipt.height() != self.context.height {
    return Err(V2BodyStoreError::KuraReceiptMismatch);
}
let directory = self.bound_directory.take().ok_or_else(|| {
    V2BodyStoreError::UnsupportedStorageBinding { path: self.directory.clone(), }
})?;
Ok(V2BodyRetirementJob { directory })
""", "body retirement consumes the exact bound directory only after matching receipt context and height")
    require("body_retirement_execute", "self.directory.retire()", "body retirement executes its already-bound directory capability")
    require("cleanup_execute", """
if let Err(error) = job.bodies.execute() {
    report_post_finality_cleanup_warning(job.identity, PostFinalityCleanupTarget::DurableBodies, &error.to_string(),);
}
""", "the bounded janitor executes durable body retirement and records retained-file failures")
    dispatch_path, dispatch_source = sources["v2_worker_completion.rs"]
    dispatch_matches = tuple(item for item in _ingress_effects_parsed_items(dispatch_source, "spawn")
        if item.brace_context == (rust_code_tokens("impl V2IoHandle"),))
    dispatch = dispatch_matches[0] if len(dispatch_matches) == 1 else None
    if dispatch is None:
        errors.append(f"{dispatch_path}: worker ACK requires the live I/O retirement dispatcher")
    _require_rust_item_context(dispatch_path, dispatch, (rust_code_tokens("impl V2IoHandle"),),
        "live I/O retirement dispatch", errors, expected_attributes=())
    _require_rust_token_sequence(dispatch_path, dispatch, """
V2IoCommand::Retire(retire) => {
    let Some(completion) = execute_retire_io_command(&output_guard, || {
        let bodies = body_store.take().expect("Retire consumes the live height-local body store")
            .into_retirement_job(&retire.receipt).map_err(|error| error.to_string())?;
        retire.cleanup.try_submit(PostFinalityCleanupJob {
            identity: CleanupWorkerIdentity::from_receipt(&retire.receipt), bodies,
        })
    }) else { break; };
""", "live retirement dispatch transfers the receipt-authenticated body store into the bounded cleanup job", errors)
    return errors


_INGRESS_EFFECTS_RECONCILIATION_OWNERS = {
    "effects::accept_payload_chunk_with_ingress_ownership": ("v2_effects.rs", "accept_payload_chunk_with_ingress_ownership", "impl<R: EffectRuntime> V2EffectExecutor<R>", ()),
    "worker::route_payload_chunk": ("v2_worker_services_impl.rs", "route_payload_chunk", "impl ProductionV2Services", ()),
    "production_exact_output_observes_finality_only_after_state_commit": ("tests/v2_worker_backpressure_retirement_cases.rs", "production_exact_output_observes_finality_only_after_state_commit", "", ("#[test]",)),
    "applied_height_finality_releases_only_covered_ticketless_payload_chunks": ("tests/v2_worker_backpressure_retirement_cases.rs", "applied_height_finality_releases_only_covered_ticketless_payload_chunks", "", ("#[test]",)),
    "predecessor_remains_exact": ("v2_effects.rs", "lifecycle_decision_apply_runtime_predecessor_remains_exact", "impl V2EffectExecutor<SerializedV2Runtime>", ()),
    "released_validate_preflight": ("v2_effects_lifecycle_admission_settlement.rs", "preflight_pending_released_validate_apply_publication", "impl V2EffectExecutor<SerializedV2Runtime>", ()),
    "validate_body": ("v2_effects_lifecycle_admission_settlement.rs", "validate_body", "impl<R: EffectRuntime> V2EffectExecutor<R>", ()),
    "test_executor_owners_empty": ("v2_effects.rs", "lifecycle_decision_apply_executor_owners_are_empty", "impl<R: EffectRuntime> V2EffectExecutor<R>", ("#[cfg(test)]",)),
    "settle_released_validate": ("v2_effects_lifecycle_admission_settlement.rs", "settle_pending_released_validate_apply_publication", "impl V2EffectExecutor<SerializedV2Runtime>", ()),
    "commit_released_validate": ("v2_effects_lifecycle_admission_settlement.rs", "commit_released_validate_apply_publication", "impl V2EffectExecutor<SerializedV2Runtime>", ()),
    "register_outbound_payload": ("v2_worker_services_impl.rs", "register_outbound_payload", "impl ProductionV2Services", ()),
    "coordinator_released_validate_preflight": ("v2_lifecycle_body_pipeline_transition.rs", "preflight_released_validate_apply_publication", "impl super::ProductionLifecycleOwnerV1", ()),
    "coordinator_released_validate_publish": ("v2_lifecycle_body_pipeline_transition.rs", "publish_released_validate_apply", "impl super::ProductionLifecycleOwnerV1", ("#[allow(clippy::too_many_lines)]",)),
}


def _require_owned_payload_chunk_envelope(
    path: Path, item: RustItem | None, worker: bool, errors: list[str],
) -> None:
    """Authenticate the moved envelope before recovering the same owned chunk."""

    if worker:
        sequence = """
let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
    wire::ConsensusMessageV2Payload::PayloadChunk(chunk),
));
if !ingress_ownership.validate_exact()
    || !ingress_ownership.matches_message(&chunk_message)
    || !ingress_ownership.matches_semantic_origin(&sender)
{
    return Err("payload chunk carried altered fair-ingress ownership".to_owned());
}
let chunk = match chunk_message {
    BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::PayloadChunk(chunk), ..
    }) => chunk,
    _ => return Err("payload chunk ownership envelope changed variant".to_owned()),
};
let manifest_hash = chunk.manifest_hash;
if let Some(work_id) = self.fetch_work_for_manifest(manifest_hash) {
    return self.deliver_payload_chunk(executor, work_id, sender, chunk, ingress_ownership);
}
"""
        label = "payload chunk routing must authenticate its moved envelope before recovering and delivering the same chunk"
    else:
        sequence = """
let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
    wire::ConsensusMessageV2Payload::PayloadChunk(chunk),
));
if !ingress_ownership.validate_exact()
    || !ingress_ownership.matches_message(&message)
    || !ingress_ownership.matches_semantic_origin(authenticated_sender)
{
    return Err(self.fail_closed_transport(
        "payload chunk lost or altered its fair-ingress ownership", services,
    ));
}
let chunk = match message {
    BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::PayloadChunk(chunk), ..
    }) => chunk,
    _ => {
        return Err(self.fail_closed_transport(
            "payload chunk ownership envelope changed variant", services,
        ));
    }
};
self.accept_payload_chunk_inner(work_id, chunk, authenticated_sender, services)
"""
        label = "payload chunk effect consumption must reject a changed envelope or semantic origin before mutation and consume that same moved chunk"
    _require_rust_token_sequence(path, item, sequence, label, errors)
    _require_rust_token_sequence(path, item,
        "self.deliver_payload_chunk(" if worker else "self.accept_payload_chunk_inner(",
        "owned payload chunk has one authenticated delivery seam", errors, count=1)


def _require_apply_tombstone_owner_census(
    path: Path, source: str, items: dict[str, tuple[Path, RustItem | None]], errors: list[str],
) -> None:
    """Account for five reviewed reads without widening finality mutation authority."""

    _require_rust_source_token_sequence(path, source, "finality_completion",
        "Apply tombstone has 23 original uses plus five individually owned reads", errors, count=28)
    _require_rust_source_token_sequence(path, source, "|| self.finality_completion.is_some()",
        "Apply tombstone has seven original and two reviewed publication/cleanup rejection guards", errors, count=9)
    _require_rust_source_token_sequence(path, source, "self.finality_completion =",
        "only runtime and lifecycle finality installation may assign the Apply tombstone", errors, count=2)
    _require_rust_source_token_sequence(path, source, "self.finality_completion = Some(FinalityCompletion {",
        "both Apply tombstone assignments retain a typed finality completion", errors, count=2)
    remainder = source
    for key, count, guard_count in (
        ("predecessor_remains_exact", 1, 0),
        ("released_validate_preflight", 1, 1),
        ("validate_body", 3, 2),
        ("test_executor_owners_empty", 1, 0),
    ):
        owner_path, item = items[key]
        _require_rust_token_sequence(owner_path, item, "finality_completion",
            f"Apply tombstone exact named owner {key}", errors, count=count)
        _require_rust_token_sequence(owner_path, item, "|| self.finality_completion.is_some()",
            f"Apply tombstone exact rejection owner {key}", errors, count=guard_count)
        if item is not None:
            if remainder.count(item.source) != 1:
                errors.append(f"{path}: Apply tombstone census must locate one exact {key} owner")
            else:
                remainder = remainder.replace(item.source, "", 1)
    # In the exact 7949245e28 preimage only validate_body existed among these
    # four owners, contributing one reference and one rejection. The remaining
    # 22 references / six rejection guards are unchanged by this reconciliation.
    _require_rust_source_token_sequence(path, remainder, "finality_completion",
        "Apply tombstone unnamed remainder stays at its original 22 references", errors, count=22)
    _require_rust_source_token_sequence(path, remainder, "|| self.finality_completion.is_some()",
        "Apply tombstone unnamed remainder keeps six original rejection guards", errors, count=6)


@lru_cache(maxsize=128)
def _ingress_effects_parsed_items(source: str, name: str) -> tuple[RustItem, ...]:
    """Cache parsing by complete source bytes and item name, including mutations."""

    return rust_items(source, name)


def _ingress_effects_source_fidelity_errors(repo_root: Path = ROOT_DIR) -> list[str]:
    """Bind moved chunk ingress and exact released-Validate finality exclusion."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    items: dict[str, tuple[Path, RustItem | None]] = {}
    auxiliary = set(_INGRESS_EFFECTS_RECONCILIATION_OWNERS) - {
        "effects::accept_payload_chunk_with_ingress_ownership", "worker::route_payload_chunk",
        "production_exact_output_observes_finality_only_after_state_commit",
        "applied_height_finality_releases_only_covered_ticketless_payload_chunks",
    }
    if set(_PRODUCTION_INGRESS_EFFECTS_RECONCILIATION_ITEM_SHA256) != auxiliary:
        errors.append("ingress-effects auxiliary seals must equal the reviewed owner inventory")
    for key, (relative, name, context, attributes) in _INGRESS_EFFECTS_RECONCILIATION_OWNERS.items():
        if relative not in sources:
            sources[relative] = _read_reviewed_rust_source(repo_root,
                f"crates/iroha_core/src/sumeragi/{relative}", errors, f"ingress-effects {relative}")
        path, source = sources[relative]
        matches = _ingress_effects_parsed_items(source, name)
        if len(matches) == 1:
            item = matches[0]
        else:
            item = _require_rust_item(path, source, name, errors)
        _require_rust_item_context(path, item, (rust_code_tokens(context),) if context else (),
            key, errors, expected_attributes=attributes)
        mapping = (_PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256 if "::" in key else
            _APPLIED_HEIGHT_TICKETLESS_FINALITY_REGRESSION_TEST_SHA256 if key not in auxiliary else
            _PRODUCTION_INGRESS_EFFECTS_RECONCILIATION_ITEM_SHA256)
        if key in mapping:
            _require_rust_item_token_sha256(path, item, mapping[key], key, errors)
        else:
            errors.append(f"{path}: ingress-effects {key} is missing its reviewed source seal")
        items[key] = (path, item)

    def require(key: str, sequence: str, label: str, count: int = 1) -> None:
        _require_rust_token_sequence(*items[key], sequence, label, errors, count=count)

    _require_owned_payload_chunk_envelope(*items["effects::accept_payload_chunk_with_ingress_ownership"], False, errors)
    _require_owned_payload_chunk_envelope(*items["worker::route_payload_chunk"], True, errors)
    _require_apply_tombstone_owner_census(*sources["v2_effects.rs"], items, errors)
    require("predecessor_remains_exact", """
if !matches!(attestation.mode(), LifecycleDecisionApplySuccessorOutputModeV1::SameBatchSuffix) {
    return Ok(false);
}
""", "Apply predecessor continuation is restricted to the attested same-batch suffix")
    require("predecessor_remains_exact", """
Ok(census_is_exact
    && self.pending_work() == self.pending_lifecycle_output_admissions.len()
    && self.pending_runner_decision_cleanup.is_none()
    && self.recovered_decision_fetch_request_index_is_exact_and_empty()
    && self.finality_completion.is_none()
    && apply_position_is_exact
    && self.runtime.lifecycle_decision_apply_runtime_predecessor_remains_exact(
        attestation.dispatch_key().lifecycle_ordinal(),
    ))
""", "Apply predecessor continuation requires exact census, no finality and the attested runtime ordinal")
    require("test_executor_owners_empty", """
self.pending_work() == 0
    && self.pending_runner_decision_cleanup.is_none()
    && self.recovered_decision_fetch_request_index_is_exact_and_empty()
    && self.retained_effect_batch.is_none()
    && self.parked_effect_batch.is_none()
    && self.finality_completion.is_none()
    && self.runtime.queued_commands() == 0
""", "test-only executor emptiness inspects all mutation owners without assigning finality")
    require("released_validate_preflight", """
if marker.owns_live_lifecycle_row()
    || marker.terminal_no_successor_ordinal().is_none()
    || marker.latest_statement.phase() != Some(wire::GlobalPhase::Commit)
    || selected_key != Some(key)
    || runtime_decision != self.protected_decision
    || !self.decision_body_drained
    || self.pending_work() != 1
    || self.pending_runner_decision_cleanup.is_some()
    || self.live_lifecycle_validate_successor.is_some()
    || self.live_lifecycle_decision_apply.is_some()
    || !self.pending_applications.is_empty()
    || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
    || self.retained_effect_batch.is_some()
    || self.parked_effect_batch.is_some()
    || self.pending_tip_recovery.is_some()
    || self.finality_completion.is_some()
""", "released Validate publication preflight permits only its exact cleaned terminal Commit owner")
    require("validate_body", """
if let Some(pending) = self.pending_released_lifecycle_validate_apply.as_ref() {
    if pending.key() != key || !pending.exactly_matches_retry(&effect, &ownership) {
        return Err(EffectExecutorError::Contract(
            "released lifecycle Validate retry changed its pending publication owner".to_owned(),
        ));
    }
    return Ok(None);
}
""", "released Validate retries retain the exact existing publication owner")
    require("validate_body", """
let projected = marker.project_retry(&effect, &ownership)
    .map_err(EffectExecutorError::Contract)?;
let incoming_pending = ownership.exact_pending_adapter_effect_binding(&effect)
""", "released Validate projects its exact retry and incoming runtime binding before reuse")
    require("validate_body", """
if marker.owns_live_lifecycle_row()
    || marker.terminal_no_successor_ordinal().is_none()
    || incoming_statement.phase() != Some(wire::GlobalPhase::Commit)
{
    self.published_lifecycle_validate_retry_markers.insert(key, projected);
    return Ok(None);
}
if self.pending_tip_recovery.is_some() || self.durable_validate_retry_seals.contains_key(&key) {
    return Err(EffectExecutorError::Contract(
        "released lifecycle Validate retained incompatible replay authority".to_owned(),
    ));
}
""", "released Validate publishes only a terminal no-successor Commit without another replay authority")
    require("validate_body", """
.exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?
    .filter(|certificate| certificate.phase == wire::GlobalPhase::Commit)
""", "released Validate requires the exact authenticated Commit certificate")
    require("validate_body", """
if self.protected_decision != Some(decision)
    || runtime_decision != Some(decision)
    || projected.owns_live_lifecycle_row()
    || projected.latest_effect != effect
    || projected.durable_receipt != durable
    || recovered_durable != durable
    || validated.durable() != &durable
    || projected.latest_statement.context_id() != round.context_id
    || projected.latest_statement.round() != certificate.round
    || projected.latest_statement.proposal_round() != certificate.proposal_round
    || projected.latest_statement.subject() != Some(certificate.subject)
    || projected.latest_statement.phase() != Some(certificate.phase)
    || projected.latest_statement.execution_commitment() != Some(certificate.execution_commitment)
""", "released Validate authenticates the exact cached Decision and all durable validation identities")
    require("validate_body", """
if self.decision_apply_dispatch_barrier_is_occupied()
    || self.live_lifecycle_validate_successor.is_some()
    || self.live_lifecycle_decision_apply.is_some()
    || !self.pending_applications.is_empty()
    || !self.recovered_decision_fetch_request_index_is_exact_and_empty()
    || self.parked_effect_batch.is_some()
    || self.finality_completion.is_some()
{
    return Err(EffectExecutorError::Contract(
        "released lifecycle Validate crossed an occupied Apply dispatch cut".to_owned(),
    ));
}
self.ensure_pending_slot()?;
self.reconcile_decision_work(decision, true, services)?;
""", "released Validate excludes competing Apply/finality before exact Decision cleanup")
    require("validate_body", """
let retained_is_current_occurrence = self.retained_effect_batch.as_ref().is_some_and(|batch| {
    batch.effects.len() == 1 && batch.effects.front().is_some_and(|owned| {
        owned.effect == effect && owned.ownership == ownership
    })
});
if self.protected_decision != Some(decision)
    || !self.decision_body_drained
    || self.pending_work() != 0
    || (self.retained_effect_batch.is_some() && !retained_is_current_occurrence)
    || self.parked_effect_batch.is_some()
    || self.pending_tip_recovery.is_some()
    || self.finality_completion.is_some()
""", "released Validate cleanup preserves only the current exact occurrence and rechecks finality")
    require("validate_body", """
let deferred = super::v2::DeferredReleasedLifecycleValidatedMarkerV1::seal_exact(
    ReleasedLifecycleValidatedMarkerSealPermitV1::new(), tag, round, subject,
    HashOf::new(&manifest), durable, validated, certificate, effect.clone(), incoming_pending,
    marker.terminal_no_successor_ordinal().expect("eligibility checked the terminal Validate locator"),
    marker.published_effect.clone(), marker.published_pending.clone(), marker.published_statement,
)
""", "released Validate seals the exact cached receipts, Commit certificate and terminal predecessor")
    require("validate_body", """
if self.published_lifecycle_validate_retry_markers.insert(key, projected).is_some() {
    return Err(EffectExecutorError::Contract(
        "released lifecycle Validate cleanup retained a competing publication marker".to_owned(),
    ));
}
assert!(self.pending_released_lifecycle_validate_apply.replace(deferred).is_none());
return Ok(None);
""", "released Validate restores only its projected terminal marker before retaining one deferred publication")
    require("settle_released_validate", """
if let Err(error) = self.preflight_pending_released_validate_apply_publication() {
    return Err(self.close(error, services));
}
match owner.preflight_released_validate_apply_publication() {
""", "released Validate settlement checks executor ownership before coordinator capacity")
    require("settle_released_validate", """
match owner.preflight_released_validate_apply_publication() {
    Ok(crate::sumeragi::v2_lifecycle_coordinator::ReleasedValidateApplyPublicationPreflightV1::Deferred,
    ) => return Ok(0),
    Ok(crate::sumeragi::v2_lifecycle_coordinator::ReleasedValidateApplyPublicationPreflightV1::Ready,
    ) => {}
    Err(reason) => {
        return Err(self.close(EffectExecutorError::Contract(reason.to_owned()), services,));
    }
}
let pending = self.pending_released_lifecycle_validate_apply.take()
""", "released Validate capacity deferral and preflight failure retain the pending owner before take")
    require("settle_released_validate", """
let pending = self.pending_released_lifecycle_validate_apply.take()
    .expect("checked released Validate Apply owner remains installed");
let key = pending.key();
let prepared = match self.runtime.prepare_released_lifecycle_validated_apply(pending) {
    Ok(prepared) => prepared,
    Err((pending, error)) => {
        assert!(self.pending_released_lifecycle_validate_apply.replace(pending).is_none());
        return Err(self.close(EffectExecutorError::Contract(format!(
            "released Validate Apply adapter preparation failed: {error}"
        )), services,));
    }
};
let (ordinal, authority) = match owner.publish_released_validate_apply(prepared) {
""", "released Validate settlement restores failed preparation and publishes durable authority before executor commit")
    require("settle_released_validate", """
let (ordinal, authority) = match owner.publish_released_validate_apply(prepared) {
    Ok(published) => published,
    Err(error) => {
        return Err(self.close(EffectExecutorError::Contract(format!(
            "released Validate Apply lifecycle publication failed: {error}"
        )), services,));
    }
};
self.commit_released_validate_apply_publication(key, ordinal, authority);
Ok(1)
""", "released Validate settlement installs only the published exact authority")
    require("commit_released_validate", """
assert_eq!(dispatch_key.lifecycle_ordinal(), ordinal);
assert_eq!(key, (certificate.proposal_round, subject));
assert_eq!(self.protected_decision, Some(decision));
assert_eq!(self.runtime.decided_body().expect("post-fsync runtime Decision remains readable"), Some(decision));
assert!(self.decision_body_drained);
assert_eq!(self.pending_work(), 0);
assert!(self.live_lifecycle_validate_successor.is_none());
assert!(self.live_lifecycle_decision_apply.is_none());
assert_eq!(self.validated_bodies.get(&key), Some(&validated_receipt));
assert_eq!(self.durable_bodies.get(&key), Some(validated_receipt.durable()));
""", "released Validate commit rechecks the exact published ordinal, Decision and durable receipts")
    require("commit_released_validate", """
let marker = self.published_lifecycle_validate_retry_markers.remove(&key)
    .expect("preflight retained the exact terminal Validate marker");
assert!(!marker.owns_live_lifecycle_row());
assert!(marker.terminal_no_successor_ordinal().is_some());
assert_eq!(marker.latest_statement.phase(), Some(wire::GlobalPhase::Commit));
self.live_lifecycle_decision_apply = Some(LiveLifecycleDecisionApplyOwnerV1 {
    dispatch_key, tag, subject, certificate, validated_receipt, decision,
});
""", "released Validate commit consumes its terminal marker and installs the typed live Apply owner")
    require("coordinator_released_validate_preflight", """
if coordinator.fault.is_some()
    || coordinator.ledger_store.is_none()
    || coordinator.lifecycle_ordinal_authority.is_none()
    || coordinator.high_water == u128::MAX
{
    return Err("released-Validate Apply owner is not durably publishable");
}
""", "released Validate coordinator requires live durable ordinal authority")
    require("coordinator_released_validate_preflight", """
if coordinator.active_lease.is_some()
    || effect_used >= effect_limit
    || !super::schema::has_lifecycle_record_capacity(coordinator.records.len(), 1)
{
    return Ok(ReleasedValidateApplyPublicationPreflightV1::Deferred);
}
Ok(ReleasedValidateApplyPublicationPreflightV1::Ready)
""", "released Validate coordinator preserves bounded effect and record capacity")
    require("coordinator_released_validate_publish", """
let candidate = prepared.project_apply_candidate(&SealedValidateApplyProjectionPermit::new(), verified)
    .map_err(ReleasedValidateApplyPublicationErrorV1::Projection)?;
if candidate.work_class != LifecycleWorkClass::Apply
    || candidate.stage.kind() != LifecycleStageKind::ApplyDecision
    || candidate.stage.predecessor_scope() != PredecessorScope::Independent
    || candidate.initial_state != InitialLifecycleState::Ready
    || !matches!(candidate.payload, DurablePayloadReference::BodyFrame(_))
{
    return Err(ReleasedValidateApplyPublicationErrorV1::InvalidStagedShape);
}
""", "released Validate coordinator projects one authenticated independent Ready Apply body frame")
    require("coordinator_released_validate_publish", """
let record_is_exact = record.physical_slots.len() == 1
    && record.owner == owner
    && record.ordinal == ordinal
    && record.key == candidate.key
    && record.work_class == LifecycleWorkClass::Apply
    && record.stage == candidate.stage
    && record.state == LifecycleState::Ready
    && staged.active_lease.is_none()
    && staged.ready_index.contains(&ordinal)
    && staged.high_water == ordinal
    && staged.records.len() == records_before.saturating_add(1)
    && staged.durable_records.len() == durable_records_before.saturating_add(1)
    && staged.key_index.get(&candidate.key) == Some(&ordinal)
    && staged.owner_index.get(&candidate.causal_root) == Some(&owner)
    && staged.durable_records.get(&ordinal).is_some_and(|metadata| {
        metadata.matches_admission(&candidate) && metadata.continuation == DurableContinuation::None
    });
if !record_is_exact {
    return Err(ReleasedValidateApplyPublicationErrorV1::InvalidStagedShape);
}
""", "released Validate coordinator admits exactly one fully indexed durable record with no continuation")
    require("coordinator_released_validate_publish", """
if !prepared.registry_work_matches(owner, ordinal, slot, digest) {
    return Err(ReleasedValidateApplyPublicationErrorV1::InvalidStagedShape);
}
""", "released Validate coordinator rejects a mismatched concrete registry address")
    require("coordinator_released_validate_publish", """
let reservation = registry.prepare_released_validate_apply_reservation(
    registry_work, terminal, address, digest, &staged, &staged_ledger,
).map_err(ReleasedValidateApplyPublicationErrorV1::Registry)?;
if coordinator.persist_exact_staged_successor_with_ordinal_reservation(&staged, &ordinal_reservation).is_err() {
    coordinator.fault = Some(CoordinatorFault::DurabilityFailure);
    return Err(ReleasedValidateApplyPublicationErrorV1::Durability);
}
let reconciliation = reservation.install_after_ledger_fsync();
*coordinator = staged;
prepared.commit_after_lifecycle_publication();
Ok((ordinal, reconciliation))
""", "released Validate coordinator reserves exact registry ownership and fsyncs before any owner installation")
    require("register_outbound_payload", """
if self.proposal_work_retired {
    return Err("Sumeragi v2 proposal work is terminal after Decision".to_owned());
}
let output_guard = Arc::clone(&self.output_guard);
let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
    "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
})?;
""", "outbound payload registration excludes retired proposal work under the fail-stop output guard")
    require("register_outbound_payload", """
if owner != self.active_tag || payload.manifest().round != expected_round {
    return Err("Sumeragi v2 outbound payload is not owned by the active reducer incarnation".to_owned(),);
}
let manifest_hash = HashOf::new(payload.manifest());
if let Some(existing) = self.outbound_chunks.get(&manifest_hash) {
    if !existing.owns_manifest(owner, payload.manifest()) {
        return Err("conflicting local Sumeragi v2 payload manifest".to_owned());
    }
    let manifest = payload.manifest().clone();
    self.outbound_chunks.retain(|hash, _| *hash == manifest_hash);
    operation.complete();
    return Ok(manifest);
}
let (validated, signed_chunks) = self.sign_payload_chunks(payload, sender)?;
""", "outbound payload exact retry preserves the matching incarnation and cached manifest before signing")
    require("register_outbound_payload", """
let (validated, signed_chunks) = self.sign_payload_chunks(payload, sender)?;
debug_assert_eq!(validated.manifest_hash(), manifest_hash);
let manifest = validated.into_manifest();
let messages = signed_chunks.into_iter().map(|chunk| {
    Self::preencode_v2_network_message(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(chunk),
    ))
}).collect::<Result<Vec<_>, _>>()?;
let retained = RetainedOutboundPayload {
    owner, round: manifest.round, subject: manifest.subject, manifest: manifest.clone(), messages,
};
""", "outbound payload storage owns signed canonical frames before ticketless fixtures clone them")
    require("register_outbound_payload", """
self.outbound_chunks.clear();
self.outbound_chunks.insert(manifest_hash, retained);
operation.complete();
Ok(manifest)
""", "outbound payload replacement installs the exact preencoded batch before completing output ownership")
    first = "production_exact_output_observes_finality_only_after_state_commit"
    require(first, "wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(", "ticketless finality fixture uses the current empty top-up/merge constructor")
    require(first, "assert!(pending.is_pending()); assert!(pending.applied_height_finality.is_none());", "Kura-only fixture leaves both pending output and finality cache unchanged")
    require(first, """
let mut state_block = service.state.block(block.as_ref().header());
let _events = state_block.apply_without_execution(&block, topology.as_ref().to_owned());
state_block.commit().expect("commit synthetic State block");
""", "ticketless finality fixture crosses the actual committed State boundary")
    require(first, "assert!(!pending.is_pending()); assert_eq!(pending.applied_height_finality.as_ref(), Some(&artifact));", "ticketless finality fixture requires exact artifact adoption after State commit")
    second = "applied_height_finality_releases_only_covered_ticketless_payload_chunks"
    require(second, """
let messages = service.outbound_chunks.get(&HashOf::new(&manifest))
    .expect("registered payload owns its exact manifest").messages.clone();
let chunk_count = messages.len();
""", "ticketless chunk fixture borrows only registered preencoded manifest-bound frames")
    require(second, "assert_eq!(ticketless_attempts, chunk_count); assert!(!ticketless.is_pending());", "ticketless chunk fixture discharges every covered frame")
    require(second, """
assert!(ticketed.is_pending(), "ticketed payload chunks stay owned");
assert_eq!(ticket_fixture.as_ref().expect("retain genuine actor ticket fixture").waiter_count(), 1);
""", "ticketless chunk fixture preserves the genuine ticketed negative control")
    require(second, """
assert!(uncovered.is_pending(), "an applied artifact cannot release another creation scope");
""", "ticketless chunk fixture preserves the uncovered-scope negative control")
    return errors


_DECIDED_BODY_SERVE_SOURCE_OWNERS = {
    "commit_certified_serve": ("v2_runner/decided_lane_recovery.rs", "commit_certified_serve", "impl ProductionDecidedLaneRecoveryDrainCommitter<'_>"),
    "bind_leader_wire": ("v2_runner/decided_lane_recovery.rs", "bind_leader_wire", "impl DecidedLaneRecoveryDrainCommitter for ProductionDecidedLaneRecoveryDrainCommitter<'_>"),
    "permits_height": ("v2_runner/decided_lane_recovery.rs", "permits_height", "impl DecidedLaneRecoveryServeScope"),
    "permits_subject": ("v2_runner/decided_lane_recovery.rs", "permits_subject", "impl DecidedLaneRecoveryServeScope"),
    "task_from_bound_ingress": ("v2_historical_body_serve.rs", "from_bound_ingress", "impl HistoricalBodyServeTask"),
    "worker_spawn": ("v2_historical_body_serve.rs", "spawn", "impl HistoricalBodyServeService"),
    "worker_try_enqueue": ("v2_historical_body_serve.rs", "try_enqueue", "impl HistoricalBodyServeService"),
    "worker_try_recv": ("v2_historical_body_serve.rs", "try_recv", "impl HistoricalBodyServeService"),
    "worker_defer_prepared": ("v2_historical_body_serve.rs", "defer_prepared", "impl HistoricalBodyServeService"),
    "worker_has_pending": ("v2_historical_body_serve.rs", "has_pending", "impl HistoricalBodyServeService"),
    "historical_body_worker": ("v2_historical_body_serve.rs", "historical_body_worker", ""),
    "cache_serve": ("v2_historical_body_serve.rs", "serve", "impl HistoricalBodyResponseCache"),
    "cache_prepare": ("v2_historical_body_serve.rs", "prepare", "impl HistoricalBodyResponseCache"),
    "proof_mint": ("v2_historical_body_serve.rs", "mint", "impl HistoricalBodyDurableSourceProof"),
    "proof_covers_message": ("v2_historical_body_serve.rs", "covers_message_in_network", "impl HistoricalBodyDurableSourceProof"),
    "validated_cached_exact_output_hash": ("v2_historical_body_serve.rs", "validated_cached_exact_output_hash", ""),
    "server_enqueue": ("v2_block_sync.rs", "try_enqueue_historical_body", "impl V2BlockSyncServer"),
    "settle_completion": ("v2_runner.rs", "settle_historical_body_serve_completion", ""),
    "post_prepared": ("v2_worker_services_impl.rs", "post_prepared_historical_body_response_on_reply_routes_with_permit", "impl ProductionV2Services"),
}


def _require_decided_certified_serve_source_contracts(
    path: Path, item: RustItem | None, errors: list[str],
) -> None:
    """Transfer the exact authorized decided request into its bounded worker."""

    for sequence, label in (
        ("""
let inbound = self.take_inbound()?;
let ingress_ownership = self.take_bound_leader_wire()?;
let authenticated_via = inbound.via().clone();
let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
""", "decided body handoff preserves the bound occurrence and authenticated hop"),
        ("""
let BlockMessage::V2(message) = message else {
    return Err(V2RunnerError::Service(
        "terminal-recovery certified Serve changed message class after authorization".to_owned(),
    ));
};
""", "decided body handoff preserves the authorized message family"),
        ("""
let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = message.payload else {
    return Err(V2RunnerError::Service(
        "terminal-recovery certified Serve changed payload after authorization".to_owned(),
    ));
};
""", "decided body handoff preserves the authorized request payload"),
        ("message.validate_version().map_err(|error| V2RunnerError::Service(error.to_string()))?;", "decided body handoff validates wire version"),
        ("""
if !scope.permits_height(request.round.height, self.executor.context().height) {
    return Err(V2RunnerError::Service(
        "terminal-recovery certified Serve crossed its authorized height scope".to_owned(),
    ));
}
if !scope.permits_subject(request.subject, self.decided_subject) {
    mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
    return Ok(());
}
let Some(reply_routes) = reply_routes else {
    mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
    return Ok(());
};
if reply_routes.semantic_target() != &sender {
    mark_leader_wire_volatile(self.receiver, &ingress_ownership)?;
    return Ok(());
}
""", "decided body handoff enforces exact height, subject and reply target before transfer"),
        ("""
let terminal_ownership = ingress_ownership.clone();
let task = HistoricalBodyServeTask::from_bound_ingress(
    request, sender, authenticated_via, reply_routes, ingress_ownership,
);
match task.and_then(|task| self.block_sync_server.try_enqueue_historical_body(task)) {
    Ok(HistoricalBodyServeAdmission::Queued) => {}
    Ok(HistoricalBodyServeAdmission::RateLimited | HistoricalBodyServeAdmission::Busy) => {
        iroha_logger::debug!(?scope,
            "retired certified body request at bounded terminal-recovery worker admission"
        );
        mark_leader_wire_volatile(self.receiver, &terminal_ownership)?;
    }
    Err(error) if is_remote_block_sync_rejection(&error) => {
        iroha_logger::debug!(?scope, %error,
            "rejected certified body request during terminal recovery"
        );
        mark_leader_wire_volatile(self.receiver, &terminal_ownership)?;
    }
    Err(error) => return Err(error.into()),
}
""", "decided body handoff retains queued ownership, retires only rejected admission and propagates local failures"),
    ):
        _require_rust_token_sequence(path, item, sequence, label, errors)
    if item is not None:
        for forbidden in ("serve_historical_body", "serve_block_sync_while_guarded", "post_durable_history_response_on_reply_routes_with_permit"):
            if forbidden in rust_code_tokens(item.body):
                errors.append(f"{path}:{item.line}: decided body work must cross only the prepared worker seam")


def _decided_body_serve_source_fidelity_errors(repo_root: Path = ROOT_DIR) -> list[str]:
    """Independently bind decided ingress through authenticated durable completion."""

    errors: list[str] = []
    sources: dict[str, tuple[Path, str]] = {}
    items: dict[str, tuple[Path, RustItem | None]] = {}
    if set(_PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256) != set(_DECIDED_BODY_SERVE_SOURCE_OWNERS):
        errors.append("decided body Serve item seal inventory must match the reviewed handoff chain")
    for key, (relative, name, context) in _DECIDED_BODY_SERVE_SOURCE_OWNERS.items():
        if relative not in sources:
            sources[relative] = _read_reviewed_rust_source(
                repo_root, f"crates/iroha_core/src/sumeragi/{relative}", errors,
                f"decided body Serve {relative}",
            )
        path, source = sources[relative]
        item = _require_rust_item(path, source, name, errors)
        expected = (rust_code_tokens(context),) if context else ()
        _require_rust_item_context(path, item, expected, key, errors, expected_attributes=())
        digest = _PRODUCTION_DECIDED_BODY_SERVE_ITEM_SHA256.get(key)
        if digest is not None:
            _require_rust_item_token_sha256(path, item, digest, key, errors)
        items[key] = (path, item)

    def require(key: str, sequence: str, label: str, count: int = 1) -> None:
        path, item = items[key]
        _require_rust_token_sequence(path, item, sequence, label, errors, count=count)

    _require_decided_certified_serve_source_contracts(*items["commit_certified_serve"], errors)
    require("bind_leader_wire", """
if !ingress_ownership.validate_exact()
    || !ingress_ownership.matches_message(inbound.message())
    || !ingress_ownership.matches_semantic_origin(inbound.sender())
    || !ingress_ownership.matches_reply_routes(inbound.reply_routes())
""", "decided Serve binds complete fair-ingress ownership before dequeue consumption")
    require("bind_leader_wire", """
self.receiver.bind_leader_wire_runtime_ownership(&mut ingress_ownership)
    .map_err(V2RunnerError::Service)?;
self.bound_leader_wire = Some(ingress_ownership);
""", "decided Serve retains the checked runtime-bound ingress carrier")
    require("permits_height", "match self { Self::Current => request == active, Self::Historical => request < active, }", "decided Serve preserves exact current versus historical height scope")
    require("permits_subject", "match self { Self::Current => request == decided, Self::Historical => true, }", "decided Serve current scope permits only its exact Decision subject")
    require("task_from_bound_ingress", """
if request.requester != recipient
    || reply_routes.semantic_target() != &recipient
    || !reply_routes.iter().any(|route| route.is_authenticated_via(&authenticated_via))
    || !ingress_ownership.validate_exact()
    || !ingress_ownership.matches_message(&exact_message)
    || !ingress_ownership.matches_semantic_origin(&recipient)
    || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
""", "body worker task authenticates exact request, hop, full routes and ingress identity")
    require("task_from_bound_ingress", """
authenticate_certified_body_request_identity(&request, &recipient)?;
let admission_plan = HistoricalBodyAdmissionPlan::from_reply_routes(&reply_routes)?;
Ok(Self { request, recipient, authenticated_via, reply_routes, admission_plan, ingress_ownership, })
""", "body worker task validates the signature before constructing the complete route-bound admission carrier")
    require("server_enqueue", """
self.historical_body_service.as_mut().ok_or_else(|| {
    V2BlockSyncError::HistoricalBodyService("historical-body worker is not installed".into(),)
})?.try_enqueue(task)
""", "body worker enqueue requires the installed service and consumes the exact task")
    require("worker_spawn", """
limits.validate()?;
let queue_capacity = limits.task_queue_capacity.get();
let (task_tx, task_rx) = mpsc::sync_channel(queue_capacity);
let (completion_tx, completion_rx) = mpsc::sync_channel(queue_capacity);
""", "body worker task and completion channels share validated finite geometry")
    require("worker_try_enqueue", """
if self.deferred_prepared.is_some() { return Ok(HistoricalBodyServeAdmission::Busy); }
let now = Instant::now();
if !self.admission.try_reserve(&task, now)? {
    return Ok(if self.admission.outstanding >= self.admission.outstanding_capacity {
        HistoricalBodyServeAdmission::Busy
    } else { HistoricalBodyServeAdmission::RateLimited },);
}
match self.task_tx.try_send(task) {
    Ok(()) => Ok(HistoricalBodyServeAdmission::Queued),
    Err(TrySendError::Full(task)) => {
        self.admission.release(&task)?;
        Ok(HistoricalBodyServeAdmission::Busy)
    }
    Err(TrySendError::Disconnected(task)) => {
        self.admission.release(&task)?;
        Err(V2BlockSyncError::HistoricalBodyWorkerDisconnected)
    }
}
""", "body worker admission reserves budgets before nonblocking send and releases exact failed charges")
    require("worker_try_recv", """
if let Some(prepared) = self.deferred_prepared.take() {
    return Ok(Some(HistoricalBodyServeCompletion::Prepared(prepared)));
}
match self.completion_rx.try_recv() {
    Ok(completion) => { self.admission.release(completion.task())?; Ok(Some(completion)) }
    Err(TryRecvError::Empty) => Ok(None),
    Err(TryRecvError::Disconnected) => { Err(V2BlockSyncError::HistoricalBodyWorkerDisconnected) }
}
""", "body completion prioritizes the exact retry and releases each worker charge once")
    require("worker_defer_prepared", """
if self.deferred_prepared.is_some() {
    return Err(V2BlockSyncError::HistoricalBodyService(
        "historical-body retry slot already owns a prepared response".into(),
    ));
}
self.deferred_prepared = Some(prepared);
""", "body completion preserves its single retained retry owner")
    require("worker_has_pending", "self.deferred_prepared.is_some() || self.admission.outstanding != 0", "body completion remains pending while retry or outstanding ingress exists")
    require("historical_body_worker", """
let result = cache.serve(kura.as_ref(), &task.request, &task.recipient, &responder_key,);
let completion = match result {
    Ok(Some((message, proof))) => {
        HistoricalBodyServeCompletion::Prepared(PreparedHistoricalBodyOutput { task, message, proof, })
    }
    Ok(None) => HistoricalBodyServeCompletion::NoResponse(task),
    Err(error) => HistoricalBodyServeCompletion::Failed(task, error),
};
""", "body worker preserves the exact authenticated task through every completion")
    require("cache_serve", "authenticate_certified_body_request_identity(request, authenticated_requester)?;", "body cache authenticates the signed requester even on cache hits")
    require("cache_serve", """
let Some(response) = build_historical_body_response(
    kura, self.network_id, request.clone(), authenticated_requester, responder_key,
)? else { return Ok(None); };
let (message, proof) = self.prepare(request, response)?;
""", "body cache obtains a canonical authenticated Kura response before preparing its proof")
    require("cache_prepare", """
let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(response)))
    .map_err(|error| V2BlockSyncError::HistoricalBodyService(error.to_string()))?;
let message = NetworkMessage::SumeragiBlock(Arc::new(wire));
let _ = message.exact_output_hash();
let proof = HistoricalBodyDurableSourceProof::mint(self.network_id, request, &message)?;
Ok((message, proof))
""", "body worker preencodes and warms the exact output hash before minting the source proof")
    require("proof_mint", """
let exact_output_hash = validated_cached_exact_output_hash(response, request, network_message)?;
Ok(Self { network_id, source_round: request.round, source_subject: request.subject,
    responder: response.responder.clone(), request_hash: HashOf::new(request), exact_output_hash, })
""", "body proof binds network, exact request, round, subject, responder and output hash")
    require("validated_cached_exact_output_hash", """
if response.request_hash != HashOf::new(request)
    || response.manifest.round != request.round
    || response.manifest.subject != request.subject
""", "body proof rejects a foreign request, source round or subject")
    require("validated_cached_exact_output_hash", "message.cached_exact_output_hash().ok_or_else(", "body proof requires the worker-warmed exact hash")
    require("proof_covers_message", "if &self.network_id != expected_network_id { return false; }", "body proof coverage preserves its exact network")
    require("proof_covers_message", """
response.request_hash == self.request_hash
    && response.manifest.round == self.source_round
    && response.manifest.subject == self.source_subject
    && response.responder == self.responder
    && message.cached_exact_output_hash() == Some(self.exact_output_hash)
""", "body proof coverage checks every immutable output identity field")
    require("settle_completion", """
let operation = output_guard.begin_fail_stop_operation().ok_or(V2RunnerError::RestartRequired)?;
let posted = services.post_prepared_historical_body_response_on_reply_routes_with_permit(
    prepared, operation.permit(),
);
match posted {
    Ok(PreparedHistoricalBodyPostOutcome::Posted) => {}
    Ok(PreparedHistoricalBodyPostOutcome::SourceRetained(prepared)) => {
        if let Err(error) = block_sync_server.defer_prepared_historical_body_output(prepared) {
            drop(operation); return Err(error.into());
        }
    }
    Err(error) => { drop(operation); return Err(V2BlockSyncError::ResponsePost(error).into()); }
}
operation.complete();
""", "body completion posts under fail-stop guard and retains an exact rejected output before completing")
    require("settle_completion", """
HistoricalBodyServeCompletion::NoResponse(task) => {
    mark_leader_wire_volatile(receiver, task.ingress_ownership())?;
}
HistoricalBodyServeCompletion::Failed(task, error) if is_remote_block_sync_rejection(&error) => {
    mark_leader_wire_volatile(receiver, task.ingress_ownership())?;
    iroha_logger::debug!(%error, "rejected historical certified body request");
}
HistoricalBodyServeCompletion::Failed(_task, error) => return Err(error.into()),
""", "body completion retires exact remote/no-response ingress and propagates local failure")
    require("post_prepared", """
let retry = prepared.clone_for_exact_output_retry();
let (peer, reply_routes, ingress_ownership, message, proof) = prepared.into_post_parts();
if !ingress_ownership.validate_exact()
    || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
    || reply_routes.semantic_target() != &peer
    || proof.network_id() != self.context.network_id
    || proof.source_round().height > self.context.height
    || proof.responder() != &self.local_peer
    || !proof.covers_message_in_network(&self.context.network_id, &message)
""", "prepared body publication authenticates ingress, full routes and non-future local durable proof")
    require("post_prepared", """
let rollover_claim = ExactOutputRolloverClaim::DurableCertifiedBodyResponse {
    scope: self.exact_output_scope(), target: peer.clone(), network_id: self.context.network_id, proof,
};
let messages = vec![message];
let peers = vec![peer];
rollover_claim.validate_fanout(&messages, &peers)?;
durable_history_source_covers(
    &messages, &rollover_claim, &self.context.network_id, self.context.height, self.kura.as_ref(),
)?;
let ownership = self.enqueue_owned_exact_reply_routes_while_guarded(
""", "prepared body publication validates its typed durable claim before guarded ownership transfer")
    require("post_prepared", "reply_routes, Some(ingress_ownership), rollover_claim, permit,", "prepared body publication transfers full routes and exact ingress under the existing permit")
    require("post_prepared", """
if ownership == ExactFanoutOwnership::SourceRetained {
    iroha_logger::debug!("retained prepared historical Sumeragi v2 body for exact-output retry");
    return Ok(super::v2_block_sync::PreparedHistoricalBodyPostOutcome::SourceRetained(retry),);
}
Ok(super::v2_block_sync::PreparedHistoricalBodyPostOutcome::Posted)
""", "prepared body publication returns its exact source on output capacity rejection")
    return errors


def _ordinary_ingress_consumer_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Read and seal the sole post-dequeue owner independently of broad checks."""

    errors: list[str] = []
    path, source = _read_reviewed_rust_source(
        repo_root,
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        errors,
        "ordinary ingress consumer source",
    )
    name = "consume_prepared_dequeued_v2_ingress"
    item = _require_rust_item(path, source, name, errors)
    _require_rust_item_context(
        path, item, (), "ordinary ingress consumer owner", errors,
        expected_attributes=("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
    )
    for digest in (
        _PRODUCTION_ORDINARY_INGRESS_CONSUMER_ITEM_SHA256,
        _PRODUCTION_EXACT_OUTPUT_ORDINARY_INGRESS_ITEM_SHA256[name],
    ):
        _require_rust_item_token_sha256(path, item, digest, name, errors)
    _require_ordinary_ingress_consumer_source_contracts(path, item, errors)
    return errors


def _require_ordinary_ingress_consumer_source_contracts(
    path: Path, item: RustItem | None, errors: list[str],
) -> None:
    """Keep exact ingress ownership through bounded body work and CommitQC reply."""

    _require_rust_token_sequence(path, item, """
match inbound.message() {
    BlockMessage::KuraReplicaAdvert(_) => {
        admit_kura_replica_advert_ingress(receiver, kura, inbound)?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
    }
    BlockMessage::LaneBlockProposal(_)
    | BlockMessage::LaneExecutablePayload(_)
    | BlockMessage::LaneBlockNewViewVote(_)
    | BlockMessage::LaneBlockNewViewCertificate(_)
    | BlockMessage::LaneBlockVote(_)
    | BlockMessage::LaneBlockQc(_)
    | BlockMessage::LaneBlockCertificate(_)
    | BlockMessage::LaneHistoricalRecoveryRequest(_)
    | BlockMessage::LaneHistoricalRecoveryResponse(_) => {
        let _ = lane_work.accept_lane_message_with_ingress_ownership(
            inbound, executor.current_tag().view(),
        )?;
        let _ = service_historical_recovery_tick(lane_work, services)?;
        finish!(ProductionPreparedOrdinaryIngressConsumptionV1::Continue);
    }
    BlockMessage::V2(_) => {}
}
let mut ingress_ownership = inbound.take_ingress_ownership()
""", "KuraReplicaAdvert ingress must bypass both consensus reducers before propagating lane ingress failure and shared recovery", errors)
    _require_rust_token_sequence(path, item, """
let authenticated_via = inbound.via().clone();
let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
if !ingress_ownership.matches_reply_routes(reply_routes.as_ref()) {
    return Err(V2RunnerError::Service(
        "global Sumeragi v2 ingress changed its authenticated reply routes".to_owned(),
    ));
}
""", "ordinary ingress must retain its authenticated hop before consuming the exact carrier and validating complete reply routes", errors)
    _require_rust_token_sequence(path, item, """
if request.round.height < executor.context().height {
    let terminal_ownership = ingress_ownership.clone();
    let task = HistoricalBodyServeTask::from_bound_ingress(
        request, sender, authenticated_via, reply_routes, ingress_ownership,
    );
    match task.and_then(|task| block_sync_server.try_enqueue_historical_body(task)) {
        Ok(HistoricalBodyServeAdmission::Queued) => {}
        Ok(HistoricalBodyServeAdmission::RateLimited | HistoricalBodyServeAdmission::Busy,) => {
            iroha_logger::debug!(
                "retired historical certified body request at the bounded worker admission gate"
            );
            mark_leader_wire_volatile(receiver, &terminal_ownership)?;
        }
        Err(error) if is_remote_block_sync_rejection(&error) => {
            iroha_logger::debug!(%error, "rejected historical certified body request");
            mark_leader_wire_volatile(receiver, &terminal_ownership)?;
        }
        Err(error) => return Err(error.into()),
    }
} else if request.round.height == executor.context().height {
""", "ordinary ingress must transfer exact request, hop, full routes and ownership to its bounded historical worker handoff; only rejected admission may retire locally and service errors must propagate", errors)
    _require_rust_token_sequence(path, item, """
let response_peer = sender.clone();
let terminal_ownership = ingress_ownership.clone();
let served = serve_block_sync_while_guarded(
    services_output_guard.as_ref(),
    || block_sync_server.serve(kura, request, &sender, local_key),
    |response, permit| {
        services.post_durable_history_response_on_reply_routes_with_permit(
            response_peer, reply_routes, ingress_ownership, response, permit,
        )
    },
);
match finalize_bound_block_sync_serve(
    served,
    || mark_leader_wire_volatile(receiver, &terminal_ownership),
    |error| {
        iroha_logger::debug!(%error, "rejected CommitQC discovery request");
    },
)? {
""", "CommitQC discovery must remain synchronous under its output guard with exact terminal ownership", errors)
    _require_rust_token_sequence(path, item, """
services.post_durable_history_response_on_reply_routes_with_permit(
    response_peer, reply_routes, ingress_ownership, response, permit,
)
""", "historical global responses preserve the complete prevalidated route set at the single synchronous CommitQC response seam", errors, count=1)
    _require_rust_token_sequence(path, item,
        "if reply_routes.semantic_target() != &sender {",
        "historical response route sets must match their authenticated semantic target", errors, count=2)
    if item is not None:
        tokens = rust_code_tokens(item.body)
        for forbidden, description in (
            ("serve_historical_body", "historical body work must remain off the ordinary actor"),
            ("PayloadManifest", "retired standalone manifest ingress must remain absent"),
        ):
            if forbidden in tokens:
                errors.append(f"{path}:{item.line}: {description}")



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
        "drive_completion_pre_gate_inner",
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
        ("turn", "LaunchedProductionLifecycleV1", "drive_completion_pre_gate_inner"),
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

    errors.extend(_lifecycle_certified_serve_reconciled_owner_errors(repo_root))
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
        ("V2LaneWorkAdapter", "accept_lane_message_owned"),
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
    ("V2LaneWorkAdapter::accept_lane_message_owned", False,
     "lane ingress must close malformed ownership and revalidate finalized proposal and Decision bodies before dispatch while preserving restart admission closure",
        """
fn accept_lane_message_owned(
    &mut self,
    inbound: InboundBlockMessage,
    ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
    active_view: wire::View,
) -> V2LaneIngressOutcome {
    let output_guard = Arc::clone(&self.output_guard);
    let Some(_permit) = output_guard.acquire() else {
        return V2LaneIngressOutcome::Rejected;
    };
    let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
    if ingress_ownership.as_ref().is_some_and(|ownership| {
        !ownership.validate_exact()
            || !ownership.matches_message(&message)
            || !ownership.matches_semantic_origin(&sender)
            || !ownership.matches_reply_routes(reply_routes.as_ref())
    }) {
        self.output_guard.close_admission_for_restart();
        return V2LaneIngressOutcome::Rejected;
    }
    let finalized_proposal = match &message {
        BlockMessage::LaneBlockProposal(proposal) => Some(proposal),
        BlockMessage::LaneExecutablePayload(payload) => Some(&payload.origin_proposal),
        _ => None,
    };
    if finalized_proposal.is_some_and(|proposal| {
        self.finalized_autonomous_ingress_payload_for_proposal_or_fail_stop(proposal)
            .is_err()
    }) {
        return V2LaneIngressOutcome::Rejected;
    }
    if self.decision_pending()
        && let BlockMessage::LaneBlockProposal(proposal) = &message
        && proposal.descriptor.proposal_height >= self.context.height
        && !self.proposal_is_bound_to_decided_carrier(proposal)
    {
        return V2LaneIngressOutcome::Rejected;
    }
    if let BlockMessage::LaneBlockProposal(proposal) = &message
        && let Some(outcome) = self.serve_durable_lane_certificate(
            proposal,
            Some(&sender),
            reply_routes,
            ingress_ownership,
        )
    {
        return outcome;
    }
    if self.decision_pending() {
        let finalized_body = match &message {
            BlockMessage::LaneBlockVote(vote) => Some(&vote.body),
            BlockMessage::LaneBlockQc(qc) => Some(&qc.body),
            BlockMessage::LaneBlockCertificate(certificate) => {
                Some(&certificate.prepare_qc.body)
            }
            _ => None,
        };
        if finalized_body.is_some_and(|body| {
            self.finalized_autonomous_ingress_payload_or_fail_stop(body)
                .is_err()
        }) {
            return V2LaneIngressOutcome::Rejected;
        }
    }
    if self.decision_pending() && !self.lane_message_is_allowed_after_decision(&message) {
        return V2LaneIngressOutcome::Rejected;
    }
    if self.output_guard.restart_required() {
        return V2LaneIngressOutcome::Rejected;
    }
    let outcome = match message {
"""),
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


_LIFECYCLE_SERVE_RECONCILED_OWNER_PATHS = {
    "turn": "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
    "height": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
    "ordinary": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
    "pending": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
    "launch": "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
}


def _lifecycle_certified_serve_reconciled_owner_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Check reviewed owner deltas independently of complete-item seals."""

    errors: list[str] = []
    sources = {}
    for role, relative in _LIFECYCLE_SERVE_RECONCILED_OWNER_PATHS.items():
        sources[role] = _read_reviewed_rust_source(
            repo_root, relative, errors, "lifecycle Serve reconciled owner",
        )
    if errors:
        return errors

    def require(role, owner, name, description, expected, *, count=1, exact=False):
        path, source = sources[role]
        if owner:
            attributes = (
                "#[allow(clippy::result_large_err)]", "#[inline(never)]",
            ) if role == "launch" else ()
            item = _require_qualified_rust_item(
                path, source, owner, name, errors, description,
                expected_attributes=attributes,
            )
        else:
            item = _require_rust_item(path, source, name, errors)
        label = "lifecycle Serve " + description
        if exact:
            _require_exact_rust_tokens(path, item, expected, label, errors)
        else:
            _require_rust_token_sequence(path, item, expected, label, errors, count=count)

    require("turn", "LaunchedProductionLifecycleV1", "drive_completion_pre_gate",
            "ordinary pre-gate cannot mint preemption", """
pub(in crate::sumeragi) fn drive_completion_pre_gate<'cursor>(
    &mut self, runner: LifecycleCurrentRunnerTurn<'cursor>,
    lane_work: &mut V2LaneWorkAdapter,
) -> ProductionLifecycleCompletionPreGateV1<'cursor> {
    self.drive_completion_pre_gate_inner(runner, lane_work, None)
}
""", exact=True)
    require("turn", "LaunchedProductionLifecycleV1", "drive_completion_pre_gate_inner",
            "physical ordinary head requires sealed preemption", """
Ok(LifecycleCompletionTakeV1::PassThrough) => {
    if proposal_sign_preemption.is_none() {
        return ProductionLifecycleCompletionPreGateV1::Ordinary(runner);
    }
    let fence = self.executor.lifecycle_reducer_fence_observation();
    match self.owner.ready_proposal_sign_preempts_bounded_producer_point(fence) {
        Ok(true) => {}
        Ok(false) => { return ProductionLifecycleCompletionPreGateV1::Ordinary(runner); }
        Err(error) => {
            iroha_logger::error!(?error, "ordinary-head Ready proposal Sign authentication failed closed");
            self.close_output_for_restart();
            return ProductionLifecycleCompletionPreGateV1::Selected(
                ProductionLifecycleCompletionSelectionV1::RestartRequired,
            );
        }
    }
}
""")
    require("turn", "LaunchedProductionLifecycleV1", "drive_completion_pre_gate_inner",
            "Certified-Serve completion transport and fail-stop publication must retain ordered marker in the exact settlement arm", """
Ok(LifecycleCompletionTakeV1::CertifiedServe(completion)) => {
    let selected = match completion.settle_deliver_and_acknowledge(&mut self.owner, &self.services) {
        Ok(crate::sumeragi::v2_worker::LifecycleCertifiedServeCompletionSettlementV1::Claimed,)
            => ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted,
        Ok(crate::sumeragi::v2_worker::LifecycleCertifiedServeCompletionSettlementV1::TerminalReplay,)
            => ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted,
        Err(reason) => {
            iroha_logger::error!(%reason, "lifecycle Certified-Serve completion failed closed");
            self.close_output_for_restart();
            ProductionLifecycleCompletionSelectionV1::RestartRequired
        }
    };
    return ProductionLifecycleCompletionPreGateV1::Selected(selected);
}
""")
    require("turn", "LaunchedProductionLifecycleV1",
            "drive_ready_completion_turn_with_required_ordinal",
            "normal Ready dispatch cannot fabricate physical Validate custody", """
Some(ordinal) => owner.dispatch_completion_requiring_ready_ordinal(
    services, executor, runner.debt(), ordinal, None,
),
None => owner.dispatch_completion_with_runner_debt(services, executor, runner.debt(),),
""")
    for role, name in (("height", "drain_lifecycle_v2_ingress"),
                       ("ordinary", "run_lifecycle_active_height")):
        require(role, None, name, "Runtime retains exact live Apply ordinal", """
advance_executor(receiver, owner, executor, services, producer_claim.required_ready_ordinal(), 1,)?
""")
    require("ordinary", None, "run_lifecycle_active_height",
            "historical completion settles before active recovery", """
let _ = settle_historical_body_serve_completion(
    receiver, block_sync_server, services, output_guard.as_ref(),
)?;
retry_recovered_decision_fetch_if_due(
""")
    require("ordinary", None, "run_lifecycle_active_height",
            "active rollover retains historical output owner", """
let finalization_ready = if ready_to_finish && !block_sync_server.has_pending_historical_body_serve() {
    activated.ready_for_finalized_rollover(&mut active_runner)?
} else { false };
""")
    require("ordinary", None, "run_lifecycle_active_height",
            "sidecar ingress needs typed permit and prepared owner", """
} else if lane_only_completion_barrier {
    if let Some(permit) = producer_claim.validate_sidecar_pacemaker_escape_permit()
        && let Some(prepared) = activated.prepare_validate_sidecar_pacemaker_ingress_turn(permit)?
    {
        let _ = activated.consume_prepared_ordinary_ingress_turn(
            &mut active_runner, prepared, &mut lane_work, kura.as_ref(),
            &common_config.key_pair, block_sync_server, block_sync,
            &mut block_sync_request, npos_beacon,
        )?;
    }
""")
    require("ordinary", None, "run_lifecycle_active_height",
            "sidecar Runtime preserves physical cut and typed escape", """
if let Some(_permit) = producer_claim.validate_sidecar_pacemaker_escape_permit() {
    executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
    let completion_cut = services.prepare_completion_runtime_cut(
        executor.remaining_completion_capacity() != 0,
    ).map_err(V2RunnerError::Service)?;
    match completion_cut {
        V2CompletionRuntimeCutDecisionV1::RetryCompletion => {}
        V2CompletionRuntimeCutDecisionV1::Runtime(completion_cut) => {
            let _ = executor.step_pacemaker_after_completion_runtime_cut(completion_cut, services,)?;
        }
        V2CompletionRuntimeCutDecisionV1::CapacityRelief(completion_cut) => {
            let _ = executor.step_completion_capacity_relief_after_cut(completion_cut, services,)?;
        }
    }
""")
    require("pending", None, "run_pending_active_height",
            "pending completion settles before no-clock Serve", """
activated.with_runner_runtime(&mut active_runner,
    |_executor, services, _lane_work| -> Result<_, V2RunnerError> {
        let _ = settle_historical_body_serve_completion(
            receiver, block_sync_server, services, output_guard.as_ref(),
        )?;
        Ok(())
    },
)?;
if let Err(error) = activated.settle_certified_serve_completion_for_no_clock_recovery(&mut active_runner)
""")
    require("pending", None, "run_pending_active_height",
            "pending rollover retains historical output owner", """
let ready = ready_to_finish && !terminal_exact_output_pending
    && !block_sync_server.has_pending_historical_body_serve();
""")
    require("pending", None, "run_pending_active_height",
            "closed-prefix drain settles historical completion first", """
let _ = settle_historical_body_serve_completion(
    receiver, block_sync_server, services, output_guard.as_ref(),
)?;
let drained = drain_decided_lane_recovery_ingress(
""")
    require("pending", None, "run_pending_active_height",
            "closed-prefix rollover cannot drop pending historical output", """
if block_sync_server.has_pending_historical_body_serve() {
    let _ = wake_rx.recv_timeout(IDLE_POLL);
    continue;
}
if !drained_terminal_ingress && !drained_terminal_relay { break; }
""")
    for role, name in (("ordinary", "run_lifecycle_active_height"),
                       ("pending", "run_pending_active_height")):
        require(role, None, name, "terminal height authenticates exact durable evidence", """
authenticate_terminal_complete_tip(
    state.as_ref(), kura.as_ref(), context, proofs_of_possession, artifact, receipt,
)?;
Ok::<_, V2RunnerError>(())
""")
        require(role, None, name, "terminal shutdown follows evidence validation", """
)?;
activated.into_clean_shutdown(&mut active_runner)?;
return Ok(HeightRunOutcome::Terminal);
}
""")
    require("launch", "ProductionLifecycleOwnerV1", "launch",
            "launch restores registered sidecar custody", """
RegisteredLifecycleValidateSidecarWaitV1::recover_at_launch(
    &mut self.coordinator, &mut self.registry,
)
""")
    require("launch", "ProductionLifecycleOwnerV1", "launch",
            "launch threads owned body store and typed service authority", """
validator_set_pops, inputs.local_peer, inputs.local_validator,
inputs.kagemusha_mint_finality_authority, inputs.key_pair, inputs.network,
body_store, payload_store_identity.clone(), inputs.state, inputs.kura, apply_service,
""")
    require("launch", "ProductionLifecycleOwnerV1", "launch",
            "launch checks output and storage identities before publication", """
if !services.matches_lifecycle_executor_output_guard(&executor)
    || !services.matches_lifecycle_body_store(&body_store_identity)
    || !services.matches_lifecycle_payload_store(&payload_store_identity)
{
    return Err(ProductionLifecycleLaunchErrorV1::OwnershipMismatch);
}
loop {
""")
    return errors


# Reviewed lifecycle loop construction and predecessor finalization owners.
_LIFECYCLE_CONSTRUCTION_RECONCILED_OWNERS = {'ordinary_loop': ('lifecycle_run_inner.rs',
                   'run_non_pending_lifecycle_loop',
                   ('HistoricalBodyServeLimits::first_release(certified_request_capacity, '
                    'network.reply_route_source_capacity(),)?',
                    'V2BlockSyncServer::new_with_historical_body_service(context.network_id, '
                    'certified_request_capacity, Arc::clone(&kura), '
                    'common_config.key_pair.clone(), limits,)',
                    'let body_store_authority = kura.mint_v2_body_store_directory_authority()',
                    'V2BodyStore::open_with_kura_authority_and_capacity(kura.as_ref(), '
                    'body_store_authority, context.clone(), signature_policy',
                    'let (exact_output_service_owner, exact_output_transport_owner) = '
                    'durable_exact_output_handoff_owner_pair();',
                    'Arc::clone(&output_guard), Arc::clone(&block_rx), '
                    'Arc::clone(&kura_replica_advert_refresh), exact_output_service_owner, '
                    ').with_kagemusha_mint_finality_authority(kagemusha_mint_finality_authority.clone());',
                    'HeightRunOutcome::Terminal => { wait_for_terminal_shutdown(context.height, '
                    'context.id(), &ingress_ready, &block_rx, &wake_rx, &shutdown_signal,); return '
                    'Ok(()); } HeightRunOutcome::Shutdown => return Ok(()),',
                    'run_lifecycle_active_height(activated, active_runner, lane_work, &context, '
                    'verified_context.proofs_of_possession(),',
                    'HeightRunOutcome::Successor(finalized) => finalized,',
                    'verified_context = finalized.verified_context;',
                    'lifecycle_storage_authority = finalized.lifecycle_storage_authority;')),
 'pending_loop': ('lifecycle_pending_kura.rs',
                  'run_pending_kura_lifecycle_height',
                  ('HistoricalBodyServeLimits::first_release(certified_request_capacity, '
                   'network.reply_route_source_capacity(),)?',
                   'V2BlockSyncServer::new_with_historical_body_service(context.network_id, '
                   'certified_request_capacity, Arc::clone(&kura), common_config.key_pair.clone(), '
                   'limits,)',
                   'let body_store_authority = kura.mint_v2_body_store_directory_authority()',
                   'V2BodyStore::open_with_kura_authority_and_capacity(kura.as_ref(), '
                   'body_store_authority, context.clone(), signature_policy',
                   'let (exact_output_service_owner, exact_output_transport_owner) = '
                   'durable_exact_output_handoff_owner_pair();',
                   'Arc::clone(&output_guard), Arc::clone(&block_rx), '
                   'Arc::clone(&kura_replica_advert_refresh), exact_output_service_owner, '
                   ').with_kagemusha_mint_finality_authority(kagemusha_mint_finality_authority.clone());',
                   'HeightRunOutcome::Terminal => { wait_for_terminal_shutdown(context.height, '
                   'context.id(), &ingress_ready, &block_rx, &wake_rx, &shutdown_signal,); return '
                   'Ok(()); } HeightRunOutcome::Shutdown => return Ok(()),',
                   'let body_store = if emergency_fast { '
                   'V2BodyStore::open_emergency_fast_read_only(',
                   'into_quarantined_recovered_startup()',
                   'run_pending_active_height(activated, active_runner, &context, '
                   'verified_context.proofs_of_possession(),',
                   'HeightRunOutcome::Successor(successor) => successor,',
                   'global_beacon_partial_signer, kagemusha_mint_finality_authority, network, '
                   'block_rx, lane_relay_rx, pending_queue_plan_admission_dirty, wake_rx,')),
 'ordinary_active': ('lifecycle_run_inner.rs',
                     'run_lifecycle_active_height',
                     ('if ready_to_finish && '
                      '!block_sync_server.has_pending_historical_body_serve() { '
                      'activated.ready_for_finalized_rollover(&mut active_runner)? } else { false '
                      '}',
                      'if ready_to_finish && !finalization_ready { let _ = '
                      'wake_rx.recv_timeout(IDLE_POLL); continue; }',
                      'if !services.matches_lifecycle_lane_work(&lane_work)',
                      'super::preflight_finalized_lane_rollover(executor, services, &mut '
                      'lane_work, &mut canonical_lane_body_recovered,)',
                      'if finalization_ready && !rollover_ready {',
                      'let drained = drain_decided_lane_recovery_ingress(receiver, executor, services, &mut '
                      'lane_work, executor.current_tag().view(), kura.as_ref(), block_sync_server, '
                      'DecidedLaneRecoveryIngressDrainMode::OpenPreflight,)',
                      'let now = Instant::now(); if now >= next_lane_retransmit { '
                      'lane_work.schedule_retransmission()?; next_lane_retransmit = '
                      'deadline_after(now, retransmit_interval); } dispatch_lane_work_effects(&mut '
                      'lane_work, services, control_queue_capacity)?; Ok::<_, '
                      'V2RunnerError>(drained.is_some())',
                      'if !drained_terminal_ingress { let _ = wake_rx.recv_timeout(IDLE_POLL); } '
                      'continue;',
                      'let mut next_recovered_decision_fetch_retransmit = deadline_after(height_started_at, retransmit_interval);',
                      'retry_recovered_decision_fetch_if_due(now, &mut next_recovered_decision_fetch_retransmit, retransmit_interval, executor, services,)?;',
                      'settle_apply_barrier_runner_decision_handoff(executor, services, local_proposal, &mut lane_work, output_guard.as_ref(), &permit,)?; let _ = reconcile_terminal_lane_output_handoffs(permit, &mut lane_work, services, control_queue_capacity,)?;',
                      'let ready_proposal_sign_preempts_producer = if executor_slice == AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary { let fence = executor.lifecycle_reducer_fence_observation(); match owner.ready_proposal_sign_preempts_bounded_producer_point(fence) { Ok(preempts) => preempts,',
                      'AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary if ready_proposal_sign_preempts_producer => { continue; }',
                      'let terminal_exact_output_pending = activated.with_runner_runtime(&mut active_runner, |_owner, _executor, services, _local_proposal| { reconcile_terminal_lane_output_handoffs(cut.decided_lane_recovery_permit(), &mut lane_work, services, control_queue_capacity,) },)?; if terminal_exact_output_pending { let _ = wake_rx.recv_timeout(IDLE_POLL); continue; } if drained_terminal_ingress || drained_terminal_relay { continue; } receiver.ensure_closed_drained_cut().map_err(V2RunnerError::Service)?;')),
 'pending_active': ('lifecycle_pending_kura.rs',
                    'run_pending_active_height',
                    ('let finalization_ready = activated.ready_for_finalized_rollover(&mut '
                     'active_runner)?;',
                     'let rollover_ready = if finalization_ready { let rollover_ready = '
                     'activated.with_runner_runtime(&mut active_runner, |executor, services, '
                     'lane_work| { super::preflight_finalized_lane_rollover(executor, services, '
                     'lane_work, &mut canonical_lane_body_recovered,) },)?; let _ = '
                     'reconcile_pending_kura_terminal_lane_output_handoffs(&mut activated, &mut '
                     'active_runner, control_queue_capacity,)?; rollover_ready } else { false };',
                     'if !rollover_ready { let _ = wake_rx.recv_timeout(IDLE_POLL); continue; } '
                     'activated.close_runner_ingress_for_finalized_drain(&mut active_runner, '
                     'receiver)?;'))}

def _lifecycle_construction_reconciled_owner_errors(root_dir: Path) -> list[str]:
    """Bind the current typed launch and closed predecessor-recovery corridor."""
    errors: list[str] = []
    for key, (relative, symbol, required) in _LIFECYCLE_CONSTRUCTION_RECONCILED_OWNERS.items():
        path = root_dir / "crates/iroha_core/src/sumeragi/v2_runner" / relative
        item = _require_rust_item(path, path.read_text(encoding="utf-8"), symbol, errors)
        _require_rust_item_context(
            path, item, (), f"lifecycle construction {key}", errors,
            expected_attributes=("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
        )
        for token in required:
            _require_rust_token_sequence(
                path, item, token, f"lifecycle construction {key} lost reviewed authority", errors,
            )
    return errors
