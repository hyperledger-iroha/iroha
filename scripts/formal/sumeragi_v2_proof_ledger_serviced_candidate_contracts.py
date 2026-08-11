# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _serviced_candidate_production_source_fidelity_errors(
    repo_root: Path,
) -> list[str]:
    """Seal the V3-decode/V4-emit producer lifecycle and runtime handoff."""

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    paths = {
        "module": base / "mod.rs",
        "store": base / "serviced_candidate_store.rs",
        "adapter": base / "v2.rs",
        "runtime": base / "v2_runtime.rs",
        "runner": base / "v2_runner.rs",
        "worker": base / "v2_worker.rs",
    }
    descriptions = {
        "module": "serviced-candidate module registration",
        "store": "V3/V4 serviced-candidate durable store",
        "adapter": "producer-continuation adapter ownership",
        "runtime": "producer-continuation serialized runtime",
        "runner": "producer-continuation high-water binding",
        "worker": "producer-continuation worker quarantine ownership",
    }
    errors: list[str] = []
    for key, path in paths.items():
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: {descriptions[key]} must be a regular file")
    if errors:
        return errors

    sources: dict[str, str] = {}
    for key, path in paths.items():
        relative = path.relative_to(repo_root).as_posix()
        _loaded_path, sources[key] = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            descriptions[key],
        )
    structural = {
        key: mask_rust_comments_and_literals(source)
        for key, source in sources.items()
    }
    _require_rust_source_token_sequence(
        paths["module"],
        sources["module"],
        "pub(crate) mod serviced_candidate_store;",
        "the private durable candidate store must remain compiled into Sumeragi",
        errors,
    )
    for literal, description in (
        (
            "const FORMAT_VERSION_V3: u16 = 3;",
            "decode-only compatibility version must remain exactly V3",
        ),
        (
            "const FORMAT_VERSION: u16 = 4;",
            "the sole emitted serviced-candidate version must remain V4",
        ),
        (
            'const FRAME_MAGIC: &[u8; 8] = b"SUMVCAND";',
            "serviced-candidate frame magic",
        ),
        (
            "pub(crate) const SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE: usize = 11;",
            "closed eleven-stage producer address geometry",
        ),
    ):
        observed = mask_rust_comments(sources["store"]).count(literal)
        if observed != 1:
            errors.append(
                f"{paths['store']}: {description} must occur exactly once in "
                f"executable source; found {observed}"
            )

    store_struct_attributes = {
        "ServicedCandidateKey": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "PersistedServicedCandidate": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "PersistedServicedCandidatesV3": (
            "#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "ProducerContinuationAddress": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "ProducerContinuationIdentity": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "ProducerContinuationHandoffToken": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        ),
        "ProducerContinuationTerminalToken": (
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "ProducerContinuationRecord": (
            "#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "PersistedProducerContinuation": (
            "#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "PersistedServicedCandidatesV4": (
            "#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]",
            "#[norito(deny_unknown_fields)]",
        ),
        "DecodedServicedCandidates": (),
        "RestoredServicedCandidates": (),
        "ServicedCandidateStore": ("#[derive(Debug)]",),
    }
    for name, expected_sha256 in (
        _SERVICED_CANDIDATE_V4_STORE_STRUCT_SHA256.items()
    ):
        items = rust_struct_items(sources["store"], name)
        if len(items) != 1:
            errors.append(
                f"{paths['store']}: require exactly one real V4 "
                f"serviced-candidate struct named {name}; found {len(items)}"
            )
            continue
        item = items[0]
        _require_rust_item_context(
            paths["store"],
            item,
            (),
            f"V4 serviced-candidate schema {name}",
            errors,
            expected_attributes=store_struct_attributes[name],
        )
        _require_rust_item_token_sha256(
            paths["store"],
            item,
            expected_sha256,
            f"V4 serviced-candidate schema {name}",
            errors,
        )

    for source_key, digests, expected_attributes in (
        (
            "adapter",
            _SERVICED_CANDIDATE_V4_ADAPTER_STRUCT_SHA256,
            {
                "SelectedProducerLifecycle": (
                    "#[derive(Clone, Debug, PartialEq, Eq)]",
                ),
                "ProducerReservationToken": (
                    "#[derive(Clone, Debug, PartialEq, Eq)]",
                ),
                "PendingProducerHandoff": (
                    "#[derive(Clone, Debug, PartialEq, Eq)]",
                ),
            },
        ),
        (
            "runtime",
            _SERVICED_CANDIDATE_V4_RUNTIME_STRUCT_SHA256,
            {
                "RuntimeDormantLocalFifoReservation": (
                    "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]",
                ),
                "BoundedIngress": (),
            },
        ),
    ):
        for name, expected_sha256 in digests.items():
            items = rust_struct_items(sources[source_key], name)
            if len(items) != 1:
                errors.append(
                    f"{paths[source_key]}: require exactly one real producer "
                    f"ownership struct named {name}; found {len(items)}"
                )
                continue
            item = items[0]
            _require_rust_item_context(
                paths[source_key],
                item,
                (),
                f"producer ownership carrier {name}",
                errors,
                expected_attributes=expected_attributes[name],
            )
            _require_rust_item_token_sha256(
                paths[source_key],
                item,
                expected_sha256,
                f"producer ownership carrier {name}",
                errors,
            )

    def collect_items(
        source_key: str,
        specs: tuple[tuple[str, str] | tuple[str, str, str], ...],
        digests: dict[str, str],
    ) -> dict[str, RustItem | None]:
        found: dict[str, RustItem | None] = {}
        for spec in specs:
            key, name = spec[:2]
            items = rust_function_items_from_structural(
                sources[source_key], structural[source_key], name
            )
            if len(spec) == 3:
                required = rust_code_tokens(spec[2])
                items = tuple(
                    item
                    for item in items
                    if _token_sequence_count(
                        rust_code_tokens(item.source), required
                    )
                    >= 1
                )
            if len(items) != 1:
                errors.append(
                    f"{paths[source_key]}: require exactly one real "
                    f"serviced-candidate function item named {name}; "
                    f"found {len(items)}"
                )
                found[key] = None
                continue
            item = items[0]
            found[key] = item
            _require_rust_item_token_sha256(
                paths[source_key],
                item,
                digests[key],
                f"V4 serviced-candidate item {key}",
                errors,
            )
        return found

    store_specs = (
        (
            "serviced_candidate_stage_for_kind_code",
            "serviced_candidate_stage_for_kind_code",
        ),
        (
            "producer_continuation_source_class_for_kind_code",
            "producer_continuation_source_class_for_kind_code",
        ),
        (
            "identity_new",
            "new",
            "candidate: ServicedCandidateKey, causal_lifecycle_key: Hash",
        ),
        (
            "identity_address",
            "address",
            "ProducerContinuationAddress { lifecycle_slot: self.lifecycle_slot",
        ),
        ("identity_has_exact_stage", "has_exact_stage"),
        (
            "record_new",
            "new",
            "status: ProducerContinuationStatus, handoff_candidates:",
        ),
        ("record_handoff_token", "handoff_token"),
        ("record_terminal_token", "terminal_token"),
        ("producer_continuations_are_valid", "producer_continuations_are_valid"),
        (
            "leader_wire_terminal_matches_runtime",
            "leader_wire_terminal_matches_runtime",
        ),
        (
            "leader_wire_control_phase_matches_candidate",
            "leader_wire_control_phase_matches_candidate",
        ),
        (
            "leader_wire_stable_terminal_matches_runtime",
            "leader_wire_stable_terminal_matches_runtime",
        ),
        ("leader_wire_load_and_reconcile", "load_and_reconcile"),
        (
            "store_open",
            "open",
            "lifecycle_capacity: usize, ) -> Result<(Self, RestoredServicedCandidates)",
        ),
        ("store_open_with_capacities", "open_with_capacities"),
        ("store_load", "load"),
        (
            "store_reserve_producer_continuation",
            "reserve_producer_continuation",
        ),
        (
            "store_persist_with_producer_continuations",
            "persist_with_producer_continuations",
        ),
        ("encode_payload_frame", "encode_payload_frame"),
        ("encode_frame_v4", "encode_frame_v4"),
        ("encode_frame_v3", "encode_frame_v3"),
        ("decode_frame", "decode_frame"),
    )
    store_items = collect_items(
        "store",
        store_specs,
        _SERVICED_CANDIDATE_V4_STORE_ITEM_SHA256,
    )
    encode_v3 = _require_rust_item(
        paths["store"], sources["store"], "encode_frame_v3", errors
    )
    _require_rust_item_context(
        paths["store"],
        encode_v3,
        (),
        "V3 compatibility encoder",
        errors,
        expected_attributes=("#[cfg(test)]",),
    )

    adapter_specs = tuple(
        (name, name)
        for name in _SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256
    )
    adapter_items = collect_items(
        "adapter",
        adapter_specs,
        _SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256,
    )
    runtime_names = {
        "dormant_completion": "completion",
        "dormant_is_local_fifo_stage": "is_local_fifo_stage",
        "install_dormant_local_fifo_reservations": (
            "install_dormant_local_fifo_reservations"
        ),
        "dormant_local_fifo_replacement": "dormant_local_fifo_replacement",
        "dormant_local_fifo_replacement_inner": (
            "dormant_local_fifo_replacement_inner"
        ),
        "occupied_with_dormant_reservations": (
            "occupied_with_dormant_reservations"
        ),
        "active_dormant_local_fifo_reservation_count": (
            "active_dormant_local_fifo_reservation_count"
        ),
        "oldest_active_lifecycle_ordinal": "oldest_active_lifecycle_ordinal",
        "with_driver_and_lifecycle_ordinals": (
            "with_driver_and_lifecycle_ordinals"
        ),
        "freeze_due_clock_owners": "freeze_due_clock_owners",
        "minimum_active_lifecycle_ordinal": "minimum_active_lifecycle_ordinal",
        "minimum_active_lifecycle_ordinal_excluding": (
            "minimum_active_lifecycle_ordinal_excluding"
        ),
        "minimum_runnable_lifecycle_ordinal": (
            "minimum_runnable_lifecycle_ordinal"
        ),
        "complete_leader_wire_runtime_owner": (
            "complete_leader_wire_runtime_owner"
        ),
        "step": "step",
        "finish_dispatched_step": "finish_dispatched_step",
        "try_step_pacemaker_escape": "try_step_pacemaker_escape",
        "dispatch_one_pacemaker_progress": (
            "dispatch_one_pacemaker_progress"
        ),
        "step_recovery": "step_recovery",
        "dispatch_one_fence_dependency": "dispatch_one_fence_dependency",
        "dispatch_one_adapter_deferred": "dispatch_one_adapter_deferred",
    }
    runtime_items = collect_items(
        "runtime",
        tuple(runtime_names.items()),
        _SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256,
    )
    runner_items = collect_items(
        "runner",
        (("run_inner", "run_inner"),),
        _SERVICED_CANDIDATE_V4_RUNNER_ITEM_SHA256,
    )

    def require_item_sequence(
        source_key: str,
        items: dict[str, RustItem | None],
        key: str,
        sequence: str,
        description: str,
    ) -> None:
        _require_rust_token_sequence(
            paths[source_key],
            items.get(key),
            sequence,
            description,
            errors,
        )

    def require_item_order(
        source_key: str,
        items: dict[str, RustItem | None],
        key: str,
        sequences: tuple[str, ...],
        description: str,
    ) -> None:
        item = items.get(key)
        if item is None:
            return
        tokens = rust_code_tokens(item.body)
        positions = [
            _token_sequence_positions(tokens, rust_code_tokens(sequence))
            for sequence in sequences
        ]
        if any(len(found) != 1 for found in positions) or any(
            left[0] >= right[0]
            for left, right in zip(positions, positions[1:])
            if left and right
        ):
            errors.append(
                f"{paths[source_key]}:{item.line}: {description} must retain "
                "the exact reviewed order"
            )

    require_item_sequence(
        "store",
        store_items,
        "serviced_candidate_stage_for_kind_code",
        """
match kind {
    0..=6 => Some(kind),
    8 => Some(7),
    9 => Some(8),
    10 => Some(9),
    14 => Some(10),
    _ => None,
}
""",
        "the producer stage projection must reject every untracked event kind",
    )
    require_item_sequence(
        "store",
        store_items,
        "producer_continuation_source_class_for_kind_code",
        """
match kind {
    0 | 6 | 9 | 10 | 14 => Some(ProducerContinuationSourceClass::Local),
    1..=5 => Some(ProducerContinuationSourceClass::ConditionalTransport),
    8 => Some(ProducerContinuationSourceClass::VolatileBody),
    _ => None,
}
""",
        "every producer stage must retain its exact physical replay class",
    )
    require_item_sequence(
        "store",
        store_items,
        "identity_new",
        """
if lifecycle_slot == 0 || admission_ordinal == 0 {
    return Err(
        "producer-continuation lifecycle slot and ordinal must be non-zero".to_owned(),
    );
}
let stage = serviced_candidate_stage_for_kind_code(candidate.kind).ok_or_else(|| {
    "producer-continuation candidate kind has no serviced stage".to_owned()
})?;
""",
        "producer identity construction must reject zero ownership and untracked stages",
    )
    require_item_sequence(
        "store",
        store_items,
        "record_new",
        """
handoff_candidates.len() > MAX_PRODUCER_CONTINUATION_HANDOFFS
    || handoff_candidates.windows(2).any(|pair| pair[0] >= pair[1])
    || handoff_candidates.iter().any(|successor| {
        !successor.has_exact_stage()
            || successor.admission_ordinal != identity.admission_ordinal
            || successor.lifecycle_slot != identity.lifecycle_slot
            || successor.causal_lifecycle_key != identity.causal_lifecycle_key
            || successor.candidate.context_id != identity.candidate.context_id
            || successor.candidate.height != identity.candidate.height
            || successor.candidate.owner != identity.candidate.owner
            || *successor == identity
    })
""",
        "producer handoffs must be finite, ordered, and inherit exact lifecycle identity",
    )
    require_item_sequence(
        "store",
        store_items,
        "producer_continuations_are_valid",
        """
identity.address() != persisted.address
    || identity.admission_ordinal == 0
    || identity.lifecycle_slot == 0
    || identity.lifecycle_slot > lifecycle_capacity
    || !identity.has_exact_stage()
    || producer_continuation_source_class_for_kind_code(identity.candidate.kind)
        != Some(record.source_class)
    || !identity.candidate.belongs_to(context_id, height, owner)
    || !identities.insert(identity)
    || !candidate_identities.insert(identity.candidate)
    || !ordinal_stages.insert((identity.admission_ordinal, identity.stage))
""",
        "decoded producer identities must preserve address, stage, geometry, and context",
    )
    require_item_sequence(
        "store",
        store_items,
        "producer_continuations_are_valid",
        """
record.status != ProducerContinuationStatus::Terminal
    && !active_ordinals.insert(identity.admission_ordinal)
""",
        "one live producer ordinal may own only one active continuation identity",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_open",
        """
let record_capacity = lifecycle_capacity
    .checked_mul(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE)
""",
        "both bounded producer tables must derive the complete eleven-stage geometry",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_open_with_capacities",
        """
if producer_continuation_capacity % SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE != 0 {
    return Err(
        "producer-continuation capacity must be an exact lifecycle-stage geometry"
            .to_owned(),
    );
}
""",
        "producer capacity must remain an exact multiple of the closed stage count",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_open_with_capacities",
        """
let max_frame_bytes = FIXED_FRAME_HEADROOM_BYTES
    .checked_add(serviced_frame_bytes)
    .and_then(|bytes| bytes.checked_add(producer_frame_bytes))
""",
        "the frame bound must charge both independent bounded tables",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_load",
        """
state.records.len() > self.serviced_capacity
    || state.producer_continuations.len() > self.producer_continuation_capacity
""",
        "load must enforce both independent table capacities",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_load",
        """
state.decision_reclaimed
    && (!state.records.is_empty() || !state.producer_continuations.is_empty())
""",
        "Decision reclamation must leave both durable tables empty",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_load",
        """
let record = if persisted.record.status == ProducerContinuationStatus::Terminal
{
    persisted.record
} else {
    ProducerContinuationRecord::new(
        persisted.record.identity,
        ProducerContinuationStatus::Reserved,
        Vec::new(),
    )
""",
        "restart must normalize executable handoff state to selector-inert Reserved",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_reserve_producer_continuation",
        """
if incumbent.identity == record.identity {
    if incumbent != &record {
        return Err(
            "an exact producer-continuation retry changed its frozen record".to_owned(),
        );
    }
    return Ok(ProducerContinuationReservation::Coalesced);
}
""",
        "retry coalescing must compare the complete frozen producer record",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_reserve_producer_continuation",
        """
if incumbent.status != ProducerContinuationStatus::Terminal
    || incumbent.identity.candidate.context_id != record.identity.candidate.context_id
    || incumbent.identity.candidate.height != record.identity.candidate.height
    || incumbent.identity.candidate.source_view >= record.identity.candidate.source_view
    || incumbent.identity.admission_ordinal >= record.identity.admission_ordinal
""",
        "slot replacement must require terminal ownership and strict view/ordinal advance",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_persist_with_producer_continuations",
        """
if decision_reclaimed && (!records.is_empty() || !producer_continuations.is_empty()) {
""",
        "persisted Decision reclamation must reject either retained table",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_persist_with_producer_continuations",
        """
record.status == ProducerContinuationStatus::Terminal
    && !records.contains_key(&record.identity.candidate)
""",
        "a durable producer terminal must retain its exact service tombstone",
    )
    require_item_sequence(
        "store",
        store_items,
        "store_persist_with_producer_continuations",
        """
let state = PersistedServicedCandidatesV4 {
    format_version: FORMAT_VERSION,
""",
        "production persistence must emit V4 only",
    )
    require_item_sequence(
        "store",
        store_items,
        "encode_payload_frame",
        """
if frame_len > max_frame_bytes {
    return Err("serviced-candidate frame exceeds its derived byte bound".to_owned());
}
""",
        "frame emission must enforce the derived maximum size",
    )
    require_item_sequence(
        "store",
        store_items,
        "encode_payload_frame",
        """
frame.extend_from_slice(Hash::new(&payload).as_ref());
frame.extend_from_slice(&payload);
""",
        "frame emission must checksum the exact canonical payload",
    )
    require_item_sequence(
        "store",
        store_items,
        "encode_frame_v4",
        "encode_payload_frame(FORMAT_VERSION, state.encode(), max_frame_bytes)",
        "production encoding must advertise V4",
    )
    require_item_sequence(
        "store",
        store_items,
        "decode_frame",
        "if !matches!(version, FORMAT_VERSION_V3 | FORMAT_VERSION)",
        "decoding must accept exactly compatibility V3 and current V4",
    )
    require_item_sequence(
        "store",
        store_items,
        "decode_frame",
        """
if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
    return Err("serviced-candidate frame length is inconsistent".to_owned());
}
let payload = &bytes[payload_offset..];
if Hash::new(payload).as_ref() != &bytes[digest_offset..payload_offset] {
    return Err("serviced-candidate snapshot checksum mismatch".to_owned());
}
""",
        "decoding must enforce exact length and checksum before schema selection",
    )
    require_item_sequence(
        "store",
        store_items,
        "decode_frame",
        """
if state.format_version != FORMAT_VERSION_V3 || state.encode() != payload {
    return Err("v3 serviced-candidate snapshot is not canonically encoded".to_owned());
}
""",
        "V3 compatibility decoding must bind its inner version and canonical bytes",
    )
    require_item_sequence(
        "store",
        store_items,
        "decode_frame",
        """
serviced_capacity: state.capacity,
producer_continuation_capacity: state.capacity,
decision_reclaimed: state.decision_reclaimed,
records: state.records,
producer_continuations: Vec::new(),
""",
        "V3 migration must map old capacity exactly and invent no producer lifecycle",
    )
    require_item_sequence(
        "store",
        store_items,
        "decode_frame",
        """
if state.format_version != FORMAT_VERSION || state.encode() != payload {
    return Err("v4 serviced-candidate snapshot is not canonically encoded".to_owned());
}
""",
        "V4 decoding must bind its inner version and canonical bytes",
    )

    require_item_sequence(
        "store",
        store_items,
        "leader_wire_terminal_matches_runtime",
        """
candidate.context_id() == context_id
    && candidate.height() == height
    && candidate.owner() == owner
    && candidate.source_view() == token.identity.view
    && terminal.source_class() == ProducerContinuationSourceClass::ConditionalTransport
    && token.source_class == super::FairV2IngressLeaderWireSourceClass::Control
    && leader_wire_control_phase_matches_candidate(token, candidate)
    && identity.causal_lifecycle_key() == runtime_owner.causal_lifecycle_key
    && identity.admission_ordinal() == runtime_owner.admission_ordinal
""",
        "leader-wire producer terminals must match context, owner, view, phase, source, and runtime",
    )

    require_item_sequence(
        "adapter",
        adapter_items,
        "candidate_lifecycle_capacity",
        """
.saturating_add(geometry.effect_work_capacity)
.saturating_add(CANDIDATE_LIFECYCLE_DURABLE_REPLAY_CAPACITY)
""",
        "lifecycle capacity must charge configured effect work and durable replay",
    )
    _require_rust_source_token_sequence(
        paths["adapter"],
        sources["adapter"],
        """
const _: () = assert!(
    SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == ServicedCandidateStage::COUNT
);
const _: () = assert!(SERVICED_CANDIDATE_STAGES_PER_LIFECYCLE == 11);
""",
        "adapter capacity must share the exact eleven-stage store geometry",
        errors,
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "serviced_candidate",
        """
append_deferred_projection_field(&mut projection, &self.wire_context.id().encode());
append_deferred_projection_u64(&mut projection, self.wire_context.height);
append_deferred_projection_field(&mut projection, &owner);
append_deferred_projection_field(&mut projection, &leader.encode());
append_deferred_projection_u64(&mut projection, source_view);
""",
        "candidate identity must bind context, height, local owner, leader, and view",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "serviced_candidate",
        """
ServicedCandidateKey::new(
    self.wire_context.id(),
    self.wire_context.height,
    owner,
    leader,
    source_view,
    target,
    phase,
    ROUTE_NEUTRAL_SERVICED_CANDIDATE_CLASS,
    deferred_event_kind(event).code(),
    evidence,
)
""",
        "candidate identity must exclude mutable scheduler priority",
    )
    _require_rust_source_token_sequence(
        paths["adapter"],
        sources["adapter"],
        "pub(crate) priority: DeferredPriority,",
        "deferred service evidence must retain its selected physical priority",
        errors,
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "ensure_serviced_candidate_capacity_before_step",
        "let capacity = self.serviced_candidate_capacity;",
        "pre-step admission must use capacity frozen at adapter construction",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "producer_parent_replay_source_for_stage",
        """
ServicedCandidateStage::ProposalReceived
| ServicedCandidateStage::VoteReceived
| ServicedCandidateStage::QuorumCertificateReceived
| ServicedCandidateStage::TimeoutVoteReceived
| ServicedCandidateStage::TimeoutCertificateReceived => {
    ProducerParentReplaySource::ConditionalResponsiveTransport
}
""",
        "transport-owned stages must remain conditional rather than forged Local roots",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "producer_parent_has_exact_local_replay_binding",
        """
ProducerParentReplaySource::ConditionalResponsiveTransport => false,
ProducerParentReplaySource::VolatileBodyReconstruction => false,
""",
        "nonlocal producer classes may not forge immediate local replay evidence",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "open_with_aggregator_and_publication_with_capacity",
        """
let restored_producer_continuation_ordinal_high_watermark = restored_producer_continuations
    .values()
    .map(|record| record.identity().admission_ordinal())
    .max();
""",
        "the immutable producer high-water must be captured before reclamation",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "dormant_local_fifo_reservations",
        """
(record.status() != ProducerContinuationStatus::Terminal).then_some(*address)
""",
        "restart coverage must validate every nonterminal producer class",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "dormant_local_fifo_reservations",
        """
ProducerParentReplaySource::ConditionalResponsiveTransport,
ProducerContinuationSourceClass::ConditionalTransport
""",
        "restart coverage must retain conditional transport ownership",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "dormant_local_fifo_reservations",
        """
ProducerParentReplaySource::VolatileBodyReconstruction,
ProducerContinuationSourceClass::VolatileBody
""",
        "restart coverage must retain volatile-body ownership",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "producer_lifecycle_slot",
        """
if existing_slot
    .replace(slot)
    .is_some_and(|existing| existing != slot)
{
    return Err("one producer lifecycle occupied multiple bounded slots".to_owned());
}
""",
        "one causal producer lifecycle must occupy one bounded slot",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "producer_lifecycle_slot",
        """
record.status() == ProducerContinuationStatus::Terminal
    && identity.admission_ordinal() < selected.admission_ordinal
    && identity.candidate().source_view() < candidate.source_view()
""",
        "the allocator may reuse only terminal older-view and older-ordinal slots",
    )
    require_item_order(
        "adapter",
        adapter_items,
        "step_with_defer_policy",
        (
            "let producer_reservation = self.reserve_selected_producer_continuation(producer_candidate)?",
            "self.ensure_serviced_candidate_capacity_before_step(&queued, serviced_candidate)",
            "self.step_reducer(event)",
            "self.record_serviced_candidate",
        ),
        "direct service must reserve and persist producer ownership before source retirement",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "step_with_defer_policy",
        """
// Every selected exact producer class reserves its immutable lifecycle
let producer_candidate = if producer_stage.is_some() {
    serviced_candidate
} else {
    None
};
""",
        "all selected producer classes must reserve a lifecycle",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "step_with_defer_policy",
        """
if self.selected_producer_lifecycle.is_some()
    && serviced_candidate.is_some()
    && locally_reconstructible_producer
    && !producer_parent_has_exact_local_replay_binding(
""",
        "only locally reconstructible producer classes require immediate local replay proof",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "drain_deferred_with_handoff_for_ordinals",
        """
let producer_continuation = self
    .deferred_producer_continuations
    .get(&deferred_ordinal)
    .cloned();
""",
        "deferred service must retain its pre-reserved exact producer owner",
    )
    require_item_order(
        "adapter",
        adapter_items,
        "drain_deferred_with_handoff_for_ordinals",
        (
            "self.ensure_serviced_candidate_capacity_before_step(&input.event, serviced_candidate)",
            "self.step_reducer(event)",
            "self.record_serviced_candidate",
            """
self.deferred_producer_continuations
    .remove(&deferred_ordinal);
if let Some(admission) = input.admission
""",
        ),
        "deferred service must record before releasing its physical producer owner",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "record_serviced_candidate",
        """
let consumes_volatile_dormant_body = matches!(
    &reservation.change,
    ProducerReservationChange::ClaimedDormant
) && token.identity().stage()
    == ServicedCandidateStage::BodyAvailable as u8;
""",
        "a restored stage-7 BodyAvailable handoff must be distinguished from durable local replay",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "record_serviced_candidate",
        """
durable_store_terminal: durable_terminal_retirement && !consumes_volatile_dormant_body,
durable_terminal_evidence: durable_terminal_evidence && !consumes_volatile_dormant_body,
""",
        "a claimed dormant stage-7 handoff may retain neither a durable terminal nor durable terminal evidence",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "record_serviced_candidate",
        """
ProducerReservationChange::Unchanged
| ProducerReservationChange::Inserted
| ProducerReservationChange::ClaimedDormant => None,
""",
        "a claimed dormant stage-7 handoff must not restore an unrelated durable predecessor",
    )
    require_item_order(
        "adapter",
        adapter_items,
        "record_serviced_candidate",
        (
            "let consumes_volatile_dormant_body = matches!",
            "let pending = PendingProducerHandoff",
            "self.pending_producer_handoffs.insert(address, pending)",
        ),
        "stage-7 volatility must be classified before the pending handoff policy is published",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "producer_handoff_evidence",
        """
Ok(if has_concrete_successor {
    ProducerContinuationHandoffEvidence::ConcreteSuccessor
} else if pending.durable_terminal_evidence {
    ProducerContinuationHandoffEvidence::DurableTerminal
} else {
    ProducerContinuationHandoffEvidence::VolatileTerminal
})
""",
        "an empty stage-7 handoff with suppressed durable evidence must classify as volatile",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "acknowledge_producer_handoff",
        """
if pending.token != token || !token.matches_reserved(&record) {
""",
        "handoff acknowledgement must consume the exact frozen record",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "acknowledge_producer_handoff",
        """
if evidence == ProducerContinuationHandoffEvidence::DurableTerminal
    && !pending.durable_terminal_evidence
""",
        "durable terminal acknowledgement must require retained exact evidence",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "acknowledge_producer_handoff",
        """
if evidence == ProducerContinuationHandoffEvidence::VolatileTerminal
    && pending.durable_terminal_evidence
""",
        "durable terminal evidence may not be weakened to volatile",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "acknowledge_producer_handoff",
        """
match pending.durable_previous.clone() {
    Some(previous) => {
        self.durable_producer_continuations
            .insert(address, previous);
    }
    None => {
        self.durable_producer_continuations.remove(&address);
    }
}
if let Err(error) = self.persist_producer_lifecycles() {
""",
        "volatile acknowledgement must remove and persist the claimed dormant producer reservation",
    )
    require_item_order(
        "adapter",
        adapter_items,
        "acknowledge_producer_handoff",
        (
            "self.terminalize_producer_continuation(Some(address))",
            "if pending.durable_store_terminal",
            "match pending.durable_previous.clone()",
            "if let Err(error) = self.persist_producer_lifecycles()",
            "self.pending_producer_handoffs.remove(&address)",
            "self.restored_dormant_producer_continuations.remove(&address)",
        ),
        "acknowledgement must retain the process terminal, persist durable stage-7 removal, and only then clear handoff metadata",
    )

    restored_stage_seven = _require_rust_item(
        paths["adapter"],
        sources["adapter"],
        "restored_body_available_reuses_logical_lifecycle_spends_one_fresh_slot_and_does_not_resurrect",
        errors,
    )
    _require_rust_item_context(
        paths["adapter"],
        restored_stage_seven,
        (
            (
                "#",
                "[",
                "cfg",
                "(",
                "test",
                ")",
                "]",
                "mod",
                "tests",
            ),
        ),
        "restored stage-7 BodyAvailable second-restart regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_token_sequence(
        paths["adapter"],
        restored_stage_seven,
        """
assert!(
    !runtime
        .driver()
        .durable_producer_continuations
        .contains_key(&restored_address),
    "the service handoff removes the restart-stable stage-7 record"
);
""",
        "the stage-7 regression must observe durable removal immediately after acknowledgement",
        errors,
    )
    _require_rust_token_sequence(
        paths["adapter"],
        restored_stage_seven,
        """
assert!(
    restarted_again.producer_continuations.is_empty()
        && restarted_again.durable_producer_continuations.is_empty()
        && restarted_again
            .restored_dormant_producer_continuations
            .is_empty(),
    "the serviced old stage cannot resurrect on a second restart"
);
""",
        "the stage-7 regression must prove that a second restart has no producer owner to reopen",
        errors,
    )
    if restored_stage_seven is not None:
        stage_seven_tokens = rust_code_tokens(restored_stage_seven.body)
        stage_seven_sequences = (
            "!runtime.driver().durable_producer_continuations.contains_key(&restored_address)",
            "drop(runtime.into_driver())",
            "let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(",
            "restarted_again.producer_continuations.is_empty() && restarted_again.durable_producer_continuations.is_empty()",
        )
        stage_seven_positions = [
            _token_sequence_positions(
                stage_seven_tokens,
                rust_code_tokens(sequence),
            )
            for sequence in stage_seven_sequences
        ]
        if any(len(found) != 1 for found in stage_seven_positions) or any(
            left[0] >= right[0]
            for left, right in zip(
                stage_seven_positions,
                stage_seven_positions[1:],
            )
            if left and right
        ):
            errors.append(
                f"{paths['adapter']}:{restored_stage_seven.line}: restored "
                "stage-7 regression must observe durable removal before "
                "dropping the first restart, then prove absence after the "
                "second restart"
            )
    require_item_order(
        "adapter",
        adapter_items,
        "drive_effects",
        (
            "let persisted = reducer::Event::Persisted { tag, id }",
            "let continuation = match self.step_reducer(persisted.clone())",
            "self.prune_ingress_records()",
            "self.reclaim_serviced_candidates()?",
            "self.record_reducer_outcome",
        ),
        "durable WAL completion must reclaim only after ingress pruning and before publication",
    )
    require_item_sequence(
        "adapter",
        adapter_items,
        "reclaim_serviced_candidates",
        """
self.serviced_candidates.clear();
self.durable_serviced_candidates.clear();
self.producer_continuations.clear();
self.durable_producer_continuations.clear();
self.restored_dormant_producer_continuations.clear();
self.deferred_producer_continuations.clear();
self.pending_producer_handoffs.clear();
""",
        "Decision reclamation must atomically retire service and producer ownership",
    )

    require_item_sequence(
        "runtime",
        runtime_items,
        "install_dormant_local_fifo_reservations",
        """
if !self.commands.is_empty()
    || self.reserved_body_available.is_some()
    || !self.dormant_local_fifo_reservations.is_empty()
""",
        "dormant FIFO ownership must install before any physical runtime work",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "occupied_with_dormant_reservations",
        """
.and_then(|occupied| occupied.checked_add(dormant))
""",
        "dormant Local owners must consume physical queue capacity",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "active_dormant_local_fifo_reservation_count",
        """
self.dormant_local_fifo_reservations
    .len()
    .checked_sub(usize::from(aliased.is_some()))
""",
        "an aliased dormant Local owner must consume exactly one physical capacity slot",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "oldest_active_lifecycle_ordinal",
        """
for reservation in &self.dormant_local_fifo_reservations {
    if reservation.admission_ordinal == 0
        || !self
            .lifecycle_ordinals
            .recognizes_minted(reservation.admission_ordinal)
            .map_err(|_| EnqueueError::FailClosed)?
    {
        return Err(EnqueueError::FailClosed);
    }
}
Ok(command_minimum)
""",
        "dormant Local owners must consume capacity and retain exact minted "
        "identity without becoming runnable global minima before materialization",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "minimum_runnable_lifecycle_ordinal",
        """
if let Some(evidence) = completion_evidence {
    if !evidence.validate_exact()
        || !self
            .ingress
            .lifecycle_ordinals
            .recognizes_minted(evidence.lifecycle_ordinal())
            .map_err(|_| EnqueueError::FailClosed)?
    {
        return Err(EnqueueError::FailClosed);
    }
    let lifecycle_ordinal = evidence.lifecycle_ordinal();
    minimum =
        Some(minimum.map_or(lifecycle_ordinal, |ordinal| ordinal.min(lifecycle_ordinal)));
}
""",
        "serviced-candidate runnable selection must admit only exact minted "
        "completion evidence into the least-owner minimum",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "dormant_local_fifo_replacement",
        """
self.dormant_local_fifo_replacement_inner(command, false)
""",
        "ordinary FIFO admission must reject reserved-body alias replacement",
    )
    require_item_order(
        "runtime",
        runtime_items,
        "dormant_local_fifo_replacement_inner",
        (
            "if self.dormant_local_fifo_reservations.contains(&expected)",
            """
if !allow_reserved_body_alias
    && self
        .reserved_body_available
        .as_ref()
        .and_then(|reservation| reservation.dormant_replacement.as_ref())
        == Some(&expected)
""",
            "return Ok(Some(expected))",
        ),
        "exact local replay must atomically replace its latent FIFO slot",
    )
    require_item_order(
        "runtime",
        runtime_items,
        "with_driver_and_lifecycle_ordinals",
        (
            "driver.dormant_local_fifo_reservations()",
            "BoundedIngress::with_lifecycle_ordinals",
            "ingress.install_dormant_local_fifo_reservations",
            "retain_effect_ownership",
        ),
        "restart must install dormant FIFO owners before startup successors",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "step",
        """
self.finish_dispatched_step(
    now,
    effects,
    effect_source,
    effect_parent,
    effect_parent_statement,
    producer_handoff,
    retained_deferred_ingress,
)
""",
        "runtime step must transfer the exact parent statement and handoff into shared completion",
    )
    require_item_order(
        "runtime",
        runtime_items,
        "finish_dispatched_step",
        (
            """
self.retain_effect_ownership(
    effect_source,
    Some(&effect_parent),
    effect_parent_statement.as_ref(),
    &effects,
)
""",
            """
if token.identity().admission_ordinal() != effect_parent.lifecycle_ordinal()
    || token.identity().causal_lifecycle_key()
        != effect_parent.causal_origin().lifecycle_key
{
    self.latch_fail_closed("producer handoff changed its selected lifecycle identity");
    return Err(RuntimeError::FailClosed);
}
""",
            "self.driver.producer_handoff_evidence(token, !effects.is_empty())",
            "self.driver.acknowledge_producer_handoff(token, evidence)",
            """
self.complete_driver_dispatch_leader_wire_owners(
    &effect_parent,
    retained_deferred_ingress,
    completed_producer_handoff,
)
""",
            "self.observe_effects(now, &effects)",
        ),
        "live dispatch completion must retain successors, acknowledge the exact producer, "
        "terminalize the selected parent before adapter-side orphans, and publish every "
        "terminal before observing effects",
    )
    require_item_order(
        "runtime",
        runtime_items,
        "dispatch_one_adapter_deferred",
        (
            "self.retain_effect_ownership",
            "token.identity().admission_ordinal() != lifecycle_owner.lifecycle_ordinal()",
            "token.identity().causal_lifecycle_key() != lifecycle_owner.causal_origin().lifecycle_key",
            "self.driver.acknowledge_producer_handoff",
        ),
        "deferred runtime service must retain successors before exact acknowledgement",
    )
    require_item_sequence(
        "runtime",
        runtime_items,
        "complete_leader_wire_runtime_owner",
        """
receipt.owner().admission_ordinal() != parent.lifecycle_ordinal()
    || receipt.owner().causal_lifecycle_key() != parent.causal_origin().lifecycle_key
""",
        "leader-wire completion must retain exact runtime ordinal and causal key",
    )
    require_item_order(
        "runner",
        runner_items,
        "run_inner",
        (
            "adapter.restored_producer_continuation_ordinal_high_watermark()",
            "lifecycle_ordinals.advance_past(high_watermark)",
            "LeaderWireLifecycleStoreGate::open",
            "lifecycle_ordinals.advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())",
            "LeaderWireIngressBinding::bind",
            "SerializedV2Runtime::new_with_lifecycle_ordinals",
        ),
        "both restored high-waters must advance the shared source before ingress/runtime construction",
    )

    def seal_regressions(
        source_key: str,
        inventory: dict[str, str],
        long_tests: set[str] = set(),
        unix_tests: set[str] = set(),
    ) -> None:
        test_module_offset = structural[source_key].find("mod tests")
        if test_module_offset < 0:
            errors.append(
                f"{paths[source_key]}: serviced-candidate regression module is missing"
            )
            return
        for name, expected_sha256 in inventory.items():
            items = rust_function_items_from_structural(
                sources[source_key], structural[source_key], name
            )
            if len(items) != 1:
                errors.append(
                    f"{paths[source_key]}: require exactly one real V4 regression "
                    f"named {name}; found {len(items)}"
                )
                continue
            item = items[0]
            item_offset = sources[source_key].find(item.source)
            expected = (
                ("#[cfg(unix)]", "#[test]")
                if name in unix_tests
                else (
                    ("#[test]", "#[allow(clippy::too_many_lines)]")
                    if name in long_tests
                    else ("#[test]",)
                )
            )
            observed_attributes = _leading_rust_attributes(
                sources[source_key],
                structural[source_key],
                item_offset,
            )
            if item_offset <= test_module_offset or observed_attributes != expected:
                errors.append(
                    f"{paths[source_key]}:{item.line}: V4 regression {name} "
                    f"must remain in the test module with attributes {expected!r}; "
                    f"found {observed_attributes!r}"
                )
            _require_rust_item_token_sha256(
                paths[source_key],
                item,
                expected_sha256,
                f"V4 serviced-candidate regression {name}",
                errors,
            )

    seal_regressions(
        "store",
        _SERVICED_CANDIDATE_V4_STORE_REGRESSION_TEST_SHA256,
        unix_tests={
            "snapshot_load_and_retire_never_follow_substituted_symlinks"
        },
    )
    seal_regressions(
        "adapter",
        _SERVICED_CANDIDATE_V4_ADAPTER_REGRESSION_TEST_SHA256,
        long_tests={
            "aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate",
            "serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims",
        },
    )
    seal_regressions(
        "runtime",
        _SERVICED_CANDIDATE_V4_RUNTIME_REGRESSION_TEST_SHA256,
    )
    seal_regressions(
        "worker",
        _SERVICED_CANDIDATE_V4_WORKER_REGRESSION_TEST_SHA256,
    )

    for source_key in ("store", "adapter", "runtime", "runner", "worker"):
        executable = structural[source_key]
        for forbidden in ("std::env", "var_os", "serde_json"):
            if forbidden in executable:
                errors.append(
                    f"{paths[source_key]}: serviced-candidate ownership may not "
                    "depend on an environment toggle or legacy codec token "
                    f"{forbidden}"
                )
    return errors


def _effect_capacity_fetch_owner_source_fidelity_errors(
    effects_path: Path,
    source: str,
    generic_executor_context: tuple[tuple[str, ...], ...],
    errors: list[str],
) -> None:
    """Bind durable producer retirement and exact Fetch-owner admission."""

    adapter_path = effects_path.with_name("v2.rs")
    if not adapter_path.is_file() or adapter_path.is_symlink():
        errors.append(
            f"{adapter_path}: durable producer tombstone source must be a regular file"
        )
    else:
        adapter_source = adapter_path.read_text(encoding="utf-8")
        deferred_exact_owners = _require_rust_item(
            adapter_path,
            adapter_source,
            "deferred_body_pipeline_completion_exact_owner_ordinals",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            deferred_exact_owners,
            (("impl", "SumeragiV2Adapter"),),
            "Busy-deferred exact completion owner inventory",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            deferred_exact_owners,
            """
input.completion_evidence.as_ref() == Some(candidate)
    && deferred_body_pipeline_completion_stage(input, tag, round, subject)
        == Some(expected_stage)
""",
            "Busy-deferred owner inventory must require the exact stage and full completion evidence",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            deferred_exact_owners,
            ".map(|input| input.admission_ordinal)",
            "Busy-deferred owner inventory must return the runtime ownership-map key",
            errors,
        )
        adapter_preflight = _require_rust_item(
            adapter_path,
            adapter_source,
            "preflight_runtime_command_admission",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            adapter_preflight,
            (("impl", "SumeragiV2Adapter"),),
            "durable producer-tombstone admission preflight",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_preflight,
            """
let serviced = self.serviced_candidates.contains_key(&key);
let matching = self
    .producer_continuations
    .iter()
    .filter(|(_, record)| record.identity().candidate() == key)
    .collect::<Vec<_>>();
""",
            "terminal preflight must join the service marker to its exact producer record",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_preflight,
            """
let identity = record.identity();
if serviced
    || record.status() != ProducerContinuationStatus::Reserved
    || !self
        .restored_dormant_producer_continuations
        .contains(address)
    || self.durable_producer_continuations.get(address) != Some(record)
{
    return Preflight::CoalesceOwned {
        causal_lifecycle_key: identity.causal_lifecycle_key(),
        admission_ordinal: identity.admission_ordinal(),
    };
}
""",
            "live and terminal producer coalescence must return the immutable retained owner",
            errors,
        )

        retire_restored_producer = _require_rust_item(
            adapter_path,
            adapter_source,
            "retire_restored_producer_continuation",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            retire_restored_producer,
            (("impl", "SumeragiV2Adapter"),),
            "persistent stage-7 producer-record retirement",
            errors,
        )
        for sequence, description in (
            (
                """
self.ensure_ingress()?;
if admission_ordinal == 0
    || producer_stage != ServicedCandidateStage::BodyAvailable as u8
    || self.selected_producer_lifecycle.is_some()
{
    return Err(self.fail_serviced_candidate_store(
        "restored producer retirement carried an invalid stage, ordinal, or active selection"
            .to_owned(),
    ));
}
""",
                "persistent producer retirement must accept only an inactive nonzero stage-7 owner",
            ),
            (
                """
let matches = self
    .producer_continuations
    .iter()
    .filter_map(|(address, record)| {
        let identity = record.identity();
        (identity.causal_lifecycle_key() == causal_lifecycle_key
    && identity.admission_ordinal() == admission_ordinal
    && identity.stage() == producer_stage)
    .then_some((*address, record.clone()))
    })
    .collect::<Vec<_>>();
""",
                "persistent producer retirement must join the exact lifecycle key, ordinal, and stage",
            ),
            (
                """
let [(address, record)] = matches.as_slice() else {
    return match matches.len() {
        0 => Ok(false),
        _ => Err(self.fail_serviced_candidate_store(
            "restored producer retirement matched multiple bounded addresses".to_owned(),
        )),
    };
};
self.persist_restored_body_producer_retirement(*address, record)?;
Ok(true)
""",
                "persistent producer retirement must select one exact record before delegating durable removal",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                retire_restored_producer,
                sequence,
                description,
                errors,
            )

        persist_restored_body_producer = _require_rust_item(
            adapter_path,
            adapter_source,
            "persist_restored_body_producer_retirement",
            errors,
        )
        _require_rust_item_context(
            adapter_path,
            persist_restored_body_producer,
            (("impl", "SumeragiV2Adapter"),),
            "shared persist-first stage-7 producer-record retirement",
            errors,
        )
        for sequence, description in (
            (
                """
if record.status() != ProducerContinuationStatus::Reserved
    || record.source_class() != ProducerContinuationSourceClass::VolatileBody
    || record.identity().address() != address
    || record.identity().stage() != ServicedCandidateStage::BodyAvailable as u8
    || self.durable_producer_continuations.get(&address) != Some(record)
    || !self
        .restored_dormant_producer_continuations
        .contains(&address)
    || self
        .deferred_producer_continuations
        .values()
        .any(|reservation| reservation.address == address)
    || self.pending_producer_handoffs.contains_key(&address)
{
    return Err(self.fail_serviced_candidate_store(
        "restored producer retirement did not own one exact dormant durable record"
            .to_owned(),
    ));
}
""",
                "persistent producer retirement must own one exact dormant durable stage-7 volatile-body record with no live alias",
            ),
            (
                """
let process_previous = self
    .producer_continuations
    .remove(&address)
    .expect("matched process producer remains present");
let durable_previous = self
    .durable_producer_continuations
    .remove(&address)
    .expect("matched durable producer remains present");
let dormant_removed = self
    .restored_dormant_producer_continuations
    .remove(&address);
debug_assert!(dormant_removed);
if let Err(reason) = self
    .serviced_candidate_store
    .persist_with_producer_continuations(
        &self.durable_serviced_candidates,
        &self.durable_producer_continuations,
        self.serviced_candidates_decision_reclaimed,
    )
{
    self.producer_continuations
        .insert(address, process_previous);
    self.durable_producer_continuations
        .insert(address, durable_previous);
    if dormant_removed {
        self.restored_dormant_producer_continuations.insert(address);
    }
    return Err(self.fail_serviced_candidate_store(reason));
}
Ok(())
""",
                "stage-7 retirement must persist process/durable/dormant removal and roll all memory back on persistence failure",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                persist_restored_body_producer,
                sequence,
                description,
                errors,
            )

        persistent_retirement_helper = _require_rust_item(
            adapter_path,
            adapter_source,
            "assert_restored_stage_seven_retirement_does_not_resurrect",
            errors,
        )
        for sequence, description in (
            (
                """
manifest: (marker != 0xBD).then_some(manifest.clone()),
""",
                "the restart regression must make BD the unique manifest-less restored Fetch case",
            ),
            (
                """
if !reserve_completion {
    assert!(
        runtime
            .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
            .expect("persist terminal restored fetch-parent retirement")
    );
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
    assert!(
        !runtime
            .driver()
            .producer_continuations
            .contains_key(&restored_address)
            && !runtime
                .driver()
                .durable_producer_continuations
                .contains_key(&restored_address)
            && !runtime
                .driver()
                .restored_dormant_producer_continuations
                .contains(&restored_address),
        "terminal fetch cancellation must remove its dormant stage-7 parent"
    );
    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after terminal restored fetch cancellation");
    assert!(restarted_again.producer_continuations.is_empty());
    return;
}
""",
                "pre-reservation restored Fetch cancellation must persist its dormant stage-7 parent before returning",
            ),
            (
                """
let retired = if materialize_before_retirement {
    runtime
        .commit_body_available(reservation)
        .expect("materialize restored completion before pipeline retirement");
    runtime
        .retire_body_pipeline_completions(restarted_tag, round, body_subject)
        .map(|retired| retired.body_available())
} else {
    runtime.retire_unpublished_body_available(restarted_tag, round, body_subject)
};
""",
                "the restart regression must exercise unpublished and queued stage-7 retirement",
            ),
            (
                """
if let Some((path, bytes)) = sabotaged_snapshot {
    assert!(
        retired.is_err(),
        "a failed durable release cannot publish volatile token retirement"
    );
    assert_eq!(
        runtime.remaining_completion_capacity(),
        capacity_before - 1,
        "failed persistence retains the exact unpublished physical owner"
    );
    assert!(runtime.driver().fail_closed);
    assert_eq!(
        runtime
            .driver()
            .producer_continuations
            .get(&restored_address),
        runtime
            .driver()
            .durable_producer_continuations
            .get(&restored_address),
        "failed persistence restores both in-memory producer aliases"
    );
    assert!(
        runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
""",
                "the injected persistence failure must retain the volatile token and restore every in-memory producer alias",
            ),
            (
                """
assert!(retired.expect("persist and retire the restored body completion"));
assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
assert!(
!runtime
    .driver()
    .producer_continuations
    .contains_key(&restored_address)
    && !runtime
        .driver()
        .durable_producer_continuations
        .contains_key(&restored_address)
    && !runtime
        .driver()
        .restored_dormant_producer_continuations
        .contains(&restored_address),
    "terminal runtime retirement must persistently release the restored producer"
);
""",
                "reserved stage-7 retirement must observe process, durable, dormant, and capacity release before reopening",
            ),
            (
                """
drop(runtime.into_driver());
let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
    directory.path().join("safety.wal"),
    verified_genesis(context()),
    Some(0),
    reducer::Generation::new(3),
    [0x11; 32],
    fingerprints(),
    Box::new(TestAggregator),
    deferred_admission_ordinals(),
)
.expect("reopen after terminal stage-7 retirement");
assert!(
    restarted_again.producer_continuations.is_empty()
        && restarted_again.durable_producer_continuations.is_empty()
        && restarted_again
            .restored_dormant_producer_continuations
            .is_empty(),
""",
                "the stage-7 retirement regression must perform a second restart from persisted state",
            ),
            (
                """
restarted_again.producer_continuations.is_empty()
    && restarted_again.durable_producer_continuations.is_empty()
    && restarted_again
        .restored_dormant_producer_continuations
        .is_empty()
""",
                "the second restart must prove that terminally retired stage-7 ownership cannot resurrect",
            ),
        ):
            _require_rust_token_sequence(
                adapter_path,
                persistent_retirement_helper,
                sequence,
                description,
                errors,
            )
        _require_rust_token_sequence(
            adapter_path,
            persistent_retirement_helper,
            """
!runtime
    .driver()
    .producer_continuations
    .contains_key(&restored_address)
    && !runtime
        .driver()
        .durable_producer_continuations
        .contains_key(&restored_address)
    && !runtime
        .driver()
        .restored_dormant_producer_continuations
        .contains(&restored_address)
""",
            "the restart regression must observe both terminal-fetch and reserved-token process/durable/dormant removal cuts",
            errors,
            count=2,
        )

        persistent_retirement_regression = _require_rust_item(
            adapter_path,
            adapter_source,
            "restored_body_available_terminal_retirement_is_persistent_before_token_release",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            persistent_retirement_regression,
            """
assert_restored_stage_seven_retirement_does_not_resurrect(0xB8, true, false, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xB9, true, true, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBA, true, false, true);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBB, false, false, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBD, false, false, false);
""",
            "the public regression must cover unpublished, materialized, failed, manifest-bound pre-reservation, and manifest-less pre-reservation stage-7 retirement",
            errors,
        )

    drain = _require_rust_item(
        effects_path,
        source,
        "drain_retained_effect_batch",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        drain,
        generic_executor_context,
        "certified-request retained-effect retry method",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        drain,
        """
let pending_work_producer = Self::pending_work_producer(&owned.effect);
match self.consume_one(owned.effect, owned.ownership, services) {
""",
        "certified-request retry must classify and dispatch the exact retained owned effect",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        drain,
        """
Err(
    EffectExecutorError::PendingWorkCapacity { .. }
    | EffectExecutorError::CertifiedRequestCapacity { .. },
) => {
    debug_assert!(pending_work_producer.is_some());
    break;
}
Err(error) => return Err(error),
""",
        "both retained-effect capacity errors must preserve the exact FIFO head before fail-closed fallback",
        errors,
    )
    if drain is not None:
        certified_capacity_count = _token_sequence_count(
            rust_code_tokens(drain.source),
            rust_code_tokens("EffectExecutorError::CertifiedRequestCapacity"),
        )
        if certified_capacity_count != 1:
            errors.append(
                f"{effects_path}:{drain.line}: retained-effect dispatch must "
                "retry CertifiedRequestCapacity exactly once beside "
                f"PendingWorkCapacity; found {certified_capacity_count} arm(s)"
            )

    begin_fetch = _require_rust_item(
        effects_path,
        source,
        "begin_fetch",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        begin_fetch,
        generic_executor_context,
        "source-faithful certified-request capacity deferral method",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments)]",),
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
} else if let Some(certificate) = certificate {
    let plan = match self.plan_certified_fetch_request(
        existing_id,
        round,
        subject,
        certificate,
        services,
    ) {
""",
        "existing ordinary Fetch Q-capacity upgrade planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let request_plan = if let Some(certificate) = certificate {
    match self.plan_certified_fetch_request(
        work.id,
        round,
        subject,
        certificate,
        services
    ) {
""",
        "genuinely new Fetch Q-capacity request planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
    iroha_logger::debug!(
        height = round.height,
        view = round.view,
        capacity,
        "deferred certified Sumeragi v2 body-fetch authority upgrade at request capacity"
    );
    return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
}
Err(error) => return Err(error),
""",
        "an existing Fetch Q-capacity upgrade must retain and retry its exact lifecycle without partial authority installation",
        errors,
        count=2,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
Err(EffectExecutorError::CertifiedRequestCapacity { capacity }) => {
    iroha_logger::debug!(
        height = round.height,
        view = round.view,
        capacity,
        "deferred certified Sumeragi v2 body fetch at request capacity"
    );
    return Err(EffectExecutorError::CertifiedRequestCapacity { capacity });
}
Err(error) => return Err(error),
""",
        "a new Fetch Q-capacity admission must retain and retry its exact lifecycle without partial authority installation",
        errors,
        count=2,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let same_lifecycle = existing.task.ownership == ownership;
if existing.task.tag != tag {
    return Err(EffectExecutorError::Contract(
        "conflicting retransmission for one body-fetch round/subject".to_owned(),
    ));
}
if !same_lifecycle {
    return Err(EffectExecutorError::Contract(
        "body-fetch retry or authority upgrade changed its exact lifecycle owner"
            .to_owned(),
    ));
}
""",
        "Fetch owner replacement must fail before request, refinement, or service planning",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
let merged_ownership = existing
    .task
    .ownership
    .rebind_same_adapter_effect(&merged_effect)
    .map_err(EffectExecutorError::Contract)?;
let merged = BodyFetchTask {
    id: existing_id,
    tag,
    round,
    subject,
    manifest: merged_manifest,
    sources: merged_sources,
    certified_request: merged_request,
    ownership: merged_ownership,
};
""",
        "coalesced Fetch retries must rebind the concrete effect while retaining the incumbent owner",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
if merged == existing.task {
    services.enqueue_body_fetch(merged).map_err(service_error)?;
    return Ok(());
}
services
    .enqueue_body_fetch(merged.clone())
    .map_err(service_error)?;
""",
        "same-owner Fetch retries and upgrades must reach the idempotent service seam after the early owner gate",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        begin_fetch,
        """
services
    .enqueue_body_fetch(merged.clone())
    .map_err(service_error)?;
if let Some(plan) = request_plan {
    self.commit_certified_fetch_request(plan);
}
self.commit_body_pipeline_owner(owner_plan);
let pending = self
    .pending_fetches
    .get_mut(&existing_id)
    .expect("serialized body-fetch owner remains present after admission");
pending.task = merged;
pending.request_hash = request_hash;
return Ok(());
""",
        "a successful same-owner Fetch authority upgrade must atomically install P/Q state and drain its retry",
        errors,
    )
    if begin_fetch is not None:
        _require_rust_item_token_sha256(
            effects_path,
            begin_fetch,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256["begin_fetch"],
            "idempotent exact Fetch admission lifecycle",
            errors,
        )
        begin_fetch_tokens = rust_code_tokens(begin_fetch.source)
        owner_barrier_tokens = rust_code_tokens("if !same_lifecycle")
        request_plan_tokens = rust_code_tokens("self.plan_certified_fetch_request(")
        refinement_tokens = rust_code_tokens(
            "existing.task.ownership.rebind_same_adapter_effect(&merged_effect)"
        )
        barrier_tokens = rust_code_tokens("if merged == existing.task")
        retry_enqueue_tokens = rust_code_tokens(
            "services.enqueue_body_fetch(merged).map_err(service_error)?"
        )
        upgrade_enqueue_tokens = rust_code_tokens(
            "services.enqueue_body_fetch(merged.clone()).map_err(service_error)?"
        )
        barrier_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(barrier_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(barrier_tokens)]
            == barrier_tokens
        ]
        retry_enqueue_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(retry_enqueue_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(retry_enqueue_tokens)]
            == retry_enqueue_tokens
        ]
        upgrade_enqueue_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(upgrade_enqueue_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(upgrade_enqueue_tokens)]
            == upgrade_enqueue_tokens
        ]
        owner_barrier_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(owner_barrier_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(owner_barrier_tokens)]
            == owner_barrier_tokens
        ]
        request_plan_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(request_plan_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(request_plan_tokens)]
            == request_plan_tokens
        ]
        refinement_positions = [
            index
            for index in range(
                len(begin_fetch_tokens) - len(refinement_tokens) + 1
            )
            if begin_fetch_tokens[index : index + len(refinement_tokens)]
            == refinement_tokens
        ]
        if not (
            len(owner_barrier_positions) == 1
            and len(request_plan_positions) == 2
            and len(refinement_positions) == 1
            and owner_barrier_positions[0] < min(request_plan_positions)
            and owner_barrier_positions[0] < refinement_positions[0]
        ):
            errors.append(
                f"{effects_path}:{begin_fetch.line}: begin_fetch must reject "
                "one foreign incumbent owner before either request planner "
                "and before candidate refinement evidence"
            )
        if not (
            len(barrier_positions) == 1
            and len(retry_enqueue_positions) == 1
            and len(upgrade_enqueue_positions) == 1
            and barrier_positions[0] < retry_enqueue_positions[0]
            and retry_enqueue_positions[0] < upgrade_enqueue_positions[0]
        ):
            errors.append(
                f"{effects_path}:{begin_fetch.line}: begin_fetch must keep one "
                "merged == existing.task barrier, one same-owner retry "
                "enqueue inside it, and one later same-owner authority-upgrade enqueue"
            )
        for forbidden_source in (
            "self.retained_effect_batch",
            "self.retain_effect_batch",
        ):
            retained_count = _token_sequence_count(
                begin_fetch_tokens,
                rust_code_tokens(forbidden_source),
            )
            if retained_count != 0:
                errors.append(
                    f"{effects_path}:{begin_fetch.line}: Q-capacity deferrals "
                    "must not mutate the outer executor's exact retained FIFO "
                    "owner inside begin_fetch; "
                    f"found {retained_count} occurrence(s) of {forbidden_source}"
                )
