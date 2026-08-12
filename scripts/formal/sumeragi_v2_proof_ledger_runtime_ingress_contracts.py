"""Leader-wire and exact-serve runtime source-fidelity contracts."""

def _leader_wire_physical_ingress_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal logical lifecycle identity apart from fresh physical queue order.

    Restart-dormant productive wire retains its durable logical token, but an
    exact replay owns a newly admitted physical carrier and the source prefix
    which existed immediately before that carrier.  The shared selector must
    therefore validate the complete durable logical owner set while choosing
    the live turn exclusively by physical carrier ordinal.
    """

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    ingress_path = base / "mod.rs"
    store_path = base / "serviced_candidate_store.rs"
    errors: list[str] = []
    for path, description in (
        (ingress_path, "fair leader-wire physical ingress implementation"),
        (store_path, "durable leader-wire logical lifecycle implementation"),
    ):
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: {description} must be a regular file")
    if errors:
        return errors

    _reviewed_ingress_path, ingress_source = _read_reviewed_rust_source(
        repo_root,
        ingress_path.relative_to(repo_root).as_posix(),
        errors,
        "fair leader-wire physical ingress implementation",
    )
    store_source = store_path.read_text(encoding="utf-8")

    gate_set = _require_rust_item(
        store_path,
        store_source,
        "ingress_scheduler_ordinals",
        errors,
    )
    _require_rust_item_context(
        store_path,
        gate_set,
        (("impl", "LeaderWireLifecycleStoreGate"),),
        "durable leader-wire active logical owner-set projection",
        errors,
    )
    _require_rust_item_token_sha256(
        store_path,
        gate_set,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "ingress_scheduler_ordinals"
        ],
        "durable leader-wire active logical owner-set projection",
        errors,
    )
    _require_rust_token_sequence(
        store_path,
        gate_set,
        """
state
    .records
    .values()
    .filter(|record| record.status == LeaderWireLifecycleStatus::Ingress)
    .map(|record| record.token.scheduler_ordinal)
    .collect()
""",
        "durable validation must expose every active logical scheduler owner",
        errors,
    )

    gate_bind = _require_rust_item(
        ingress_path,
        ingress_source,
        "bind_leader_wire_lifecycle_gate",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        gate_bind,
        (("impl", "FairV2Ingress"),),
        "restart-restored leader-wire physical high-water binding",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        gate_bind,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "bind_leader_wire_lifecycle_gate"
        ],
        "restart-restored leader-wire physical high-water binding",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        gate_bind,
        """
!token.validate_exact(
    context_id,
    height,
    &state.roster,
    state.leader_wire_max_chunk_count,
) || token.admission_ordinal > restore.last_admission_ordinal()
""",
        "every restored durable token must remain at or below the restored "
        "physical admission high-watermark",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        gate_bind,
        """
state.last_admission_ordinal = state
    .last_admission_ordinal
    .max(restore.last_admission_ordinal());
""",
        "restart binding must preserve the durable physical admission "
        "high-watermark before any fresh carrier allocation",
        errors,
    )
    if gate_bind is not None:
        gate_bind_tokens = rust_code_tokens(gate_bind.body)
        gate_bind_sequences = (
            "token.admission_ordinal > restore.last_admission_ordinal()",
            "lifecycle_ordinals.advance_past(restore.scheduler_ordinal_high_watermark())?;",
            """
state.last_admission_ordinal = state
    .last_admission_ordinal
    .max(restore.last_admission_ordinal());
""",
            "state.leader_wire_lifecycles = records;",
        )
        gate_bind_positions = [
            _token_sequence_positions(
                gate_bind_tokens,
                rust_code_tokens(sequence),
            )
            for sequence in gate_bind_sequences
        ]
        if any(len(found) != 1 for found in gate_bind_positions) or any(
            left[0] >= right[0]
            for left, right in zip(
                gate_bind_positions,
                gate_bind_positions[1:],
            )
            if left and right
        ):
            errors.append(
                f"{ingress_path}:{gate_bind.line}: restart binding must "
                "validate every durable token before publishing the restored "
                "physical high-watermark and lifecycle map in the exact "
                "reviewed order"
            )

    dormant_admission = _require_rust_item(
        ingress_path,
        ingress_source,
        "fair_v2_ingress_admit_leader_wire",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        dormant_admission,
        (),
        "atomic leader-wire lifecycle admission",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        dormant_admission,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "fair_v2_ingress_admit_leader_wire"
        ],
        "atomic leader-wire lifecycle admission",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        dormant_admission,
        """
if incumbent_status == FairV2IngressLeaderWireStatus::Dormant && publish_ingress {
    let ingress_predecessors = state
        .lanes
        .iter()
        .map(|(source, lane)| (source.clone(), lane.entries.len()))
        .collect();
    let receipt = gate
        .admit_ingress(incumbent_token.clone())
""",
        "Dormant replay must freeze the complete current physical source prefix "
        "before durable reactivation",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        dormant_admission,
        """
let admission_ordinal = state
    .last_admission_ordinal
    .checked_add(1)
    .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;
""",
        "fresh leader-wire lifecycle admission must use the next physical "
        "high-watermark ordinal",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        dormant_admission,
        """
let token = FairV2IngressLeaderWireToken {
    source_class: identity.phase.source_class(),
    identity,
    slot: slot.clone(),
    admission_ordinal,
    scheduler_ordinal,
};
""",
        "fresh leader-wire tokens must retain the exact next physical "
        "admission ordinal separately from their scheduler ordinal",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        dormant_admission,
        """
incumbent.status = FairV2IngressLeaderWireStatus::Ingress;
incumbent.ingress_predecessors = ingress_predecessors;
""",
        "Dormant replay must install its freshly frozen physical prefix",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        dormant_admission,
        """
let ingress_predecessors = state
    .lanes
    .iter()
    .map(|(source, lane)| {
        (
            source.clone(),
            lane.entries
                .iter()
                .filter(|entry| entry.admission_ordinal < admission_ordinal)
                .count(),
        )
    })
    .collect();
""",
        "fresh leader-wire admission must freeze every earlier physical source owner",
        errors,
    )

    ordinary_selector = _require_rust_item(
        ingress_path,
        ingress_source,
        "try_recv_if_at_checked",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        ordinary_selector,
        (("impl", "FairV2Ingress"),),
        "ordinary physical fair-ingress selector wrapper",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        ordinary_selector,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "try_recv_if_at_checked"
        ],
        "ordinary physical fair-ingress selector wrapper",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        ordinary_selector,
        """
self.try_recv_if_at_checked_classified(
    service_attempt_at,
    false,
    FairV2IngressBarrierBypass::None,
    predicate,
)
""",
        "ordinary timestamped ingress must delegate with no barrier bypass",
        errors,
    )

    projection = _require_rust_item(
        ingress_path,
        ingress_source,
        "fair_v2_ingress_leader_wire_selector_projection",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        projection,
        (),
        "shared physical leader-wire selector projection",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        projection,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "fair_v2_ingress_leader_wire_selector_projection"
        ],
        "shared physical leader-wire selector projection",
        errors,
    )

    queue_gate = _require_rust_item(
        ingress_path,
        ingress_source,
        "fair_v2_ingress_queue_gate_verdict",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        queue_gate,
        (),
        "queue-local leader-wire and Serve gate verdict",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        queue_gate,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "fair_v2_ingress_queue_gate_verdict"
        ],
        "queue-local leader-wire and Serve gate verdict",
        errors,
    )

    selector = _require_rust_item(
        ingress_path,
        ingress_source,
        "try_recv_if_at_checked_classified",
        errors,
    )
    _require_rust_item_context(
        ingress_path,
        selector,
        (("impl", "FairV2Ingress"),),
        "shared physical fair-ingress selector",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        selector,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "try_recv_if_at_checked_classified"
        ],
        "shared physical fair-ingress selector",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        selector,
        """
let leader_wire_projection = fair_v2_ingress_leader_wire_selector_projection(
    &state,
    selected_serve_barrier,
    retire_obsolete_leader_wire,
    None,
)?;
""",
        "the selector must freeze one exact leader-wire projection while the queue is locked",
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
""",
        "every candidate must use the sealed queue-local gate verdict",
        errors,
    )
    _require_rust_token_sequence(
        ingress_path,
        projection,
        """
let control_barrier = selected_barrier.as_ref().is_some_and(|owner| {
    owner.token.source_class == FairV2IngressLeaderWireSourceClass::Control
});
""",
        "only an exact Control-class leader-wire owner may authorize dependency bypass",
        errors,
    )
    for sequence, description in (
        (
            """
let gate = gate
    .as_ref()
    .ok_or_else(|| "leader-wire selector crossed an unbound durable gate".to_owned())?;
let durable_ingress_ordinals = gate.ingress_scheduler_ordinals()?;
let active_ordinals = active_leader_wire_owners
    .iter()
    .map(|record| record.token.scheduler_ordinal)
    .collect::<BTreeSet<_>>();
if durable_ingress_ordinals != active_ordinals {
""",
            "the selector must compare the complete durable and in-memory "
            "logical Ingress owner sets",
        ),
        (
            """
for entry in state.lanes.values().flat_map(|lane| lane.entries.iter()) {
    let Some(token) = entry.leader_wire_token.as_ref() else {
        continue;
    };
    if carrier_ordinals
        .insert(token.clone(), entry.admission_ordinal)
        .is_some()
""",
            "every physical leader-wire carrier must inject into one token owner",
        ),
        (
            """
let carrier_ordinal = carrier_ordinals
    .remove(&owner.token)
    .ok_or_else(|| "leader-wire selector lost its exact fair-ingress carrier".to_owned())?;
active_carriers.push((owner, carrier_ordinal));
""",
            "every active logical Ingress owner must consume its one exact "
            "physical carrier",
        ),
        (
            """
if !carrier_ordinals.is_empty() {
    return Err("leader-wire carrier has no matching active lifecycle owner".to_owned());
}
active_carriers
    .retain(|(_, ordinal)| physical_cut.is_none_or(|cut| u128::from(*ordinal) < cut));
active_carriers.sort_by_key(|(_, ordinal)| *ordinal);
""",
            "carrier correspondence must be total before ordering by physical ordinal",
        ),
        (
            """
match active_carriers.first() {
    Some((owner, carrier_ordinal)) => (Some(owner.clone()), Some(*carrier_ordinal)),
    None => (None, None),
}
""",
            "the active leader-wire turn must be the minimum physical carrier",
        ),
        (
            """
selected_carrier_ordinal
    .is_some_and(|leader_ordinal| serve.carrier_ordinal() <= leader_ordinal)
""",
            "Serve-versus-leader arbitration must compare physical carrier ordinals",
        ),
        (
            """
index
    < owner
        .ingress_predecessors
        .get(source)
        .copied()
        .unwrap_or(0)
    || (owner
        .ingress_predecessors
        .values()
        .all(|count| *count == 0)
        && entry.leader_wire_token.as_ref() == Some(&owner.token))
""",
            "the restored leader-wire self-carrier must remain behind every frozen source-prefix predecessor",
        ),
    ):
        _require_rust_token_sequence(
            ingress_path,
            queue_gate if "source-prefix" in description else projection,
            sequence,
            description,
            errors,
        )
    if projection is not None and _token_sequence_count(
        rust_code_tokens(projection.source),
        rust_code_tokens(
            ".min_by_key(|record| record.token.scheduler_ordinal)"
        ),
    ):
        errors.append(
            f"{ingress_path}:{selector.line}: shared leader-wire ingress may "
            "not select by retained logical scheduler ordinal"
        )

    test_context = (
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
    regression_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _LEADER_WIRE_PHYSICAL_INGRESS_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(ingress_path, ingress_source, name, errors)
        regression_items[name] = item
        _require_rust_item_context(
            ingress_path,
            item,
            test_context,
            f"leader-wire physical-ingress regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            ingress_path,
            item,
            expected_sha256,
            f"leader-wire physical-ingress regression {name}",
            errors,
        )

    restored_retry = regression_items.get(
        "restored_productive_retry_freezes_the_current_physical_source_prefix"
    )
    for sequence, description in (
        (
            """
let earlier = v2_commit_certificate_request(0, &fixture.validator);
assert!(matches!(
    fixture.ingress.try_push(InboundBlockMessage::new(
        earlier,
        Some(fixture.validator.clone()),
    )),
    Ok(super::FairV2IngressPushDisposition::Enqueued)
));
let earlier_ordinal = fixture
    .ingress
    .state
    .lock()
    .lanes
    .values()
    .flat_map(|lane| lane.entries.iter())
    .find(|entry| entry.leader_wire_token.is_none())
    .expect("ordinary traffic owns its physical occurrence")
    .admission_ordinal;
assert!(matches!(
    fixture.ingress.try_push(InboundBlockMessage::new(
        fixture.message.clone(),
        Some(fixture.validator.clone()),
    )),
    Ok(super::FairV2IngressPushDisposition::Enqueued)
));
let retry_ordinal = fixture
    .ingress
    .state
    .lock()
    .lanes
    .values()
    .flat_map(|lane| lane.entries.iter())
    .find(|entry| entry.leader_wire_token.as_ref() == Some(&fixture.token))
    .expect("restored lifecycle acquired one fresh carrier")
    .admission_ordinal;
assert!(earlier_ordinal < retry_ordinal);
""",
            "the restart regression must admit an ordinary physical predecessor "
            "before the restored lifecycle's fresh carrier",
        ),
        (
            """
fixture
    .ingress
    .try_recv_if(|inbound| {
        inbound
            .ingress_ownership()
            .is_some_and(|ownership| ownership.leader_wire_token().is_some())
    })
    .is_none()
""",
            "the restart regression must reject target-only selection while its "
            "frozen physical predecessor remains",
        ),
        (
            """
let first = fixture
    .ingress
    .try_recv_if(|_| true)
    .expect("the replay-frozen physical predecessor drains first");
assert!(matches!(
    first.message(),
    BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(_),
        ..
    })
));
""",
            "the restart regression must drain and identify the frozen ordinary "
            "predecessor before the replay",
        ),
        (
            """
let replay = fixture
    .ingress
    .try_recv_if(|_| true)
    .expect("the exact replay drains after its frozen source prefix");
assert!(
    replay
        .ingress_ownership()
        .is_some_and(|ownership| {
            ownership.leader_wire_token() == Some(&fixture.token)
        })
);
""",
            "the restart regression must drain the exact restored owner only "
            "after its frozen physical prefix",
        ),
    ):
        _require_rust_token_sequence(
            ingress_path,
            restored_retry,
            sequence,
            description,
            errors,
        )

    return errors


def _outer_ingress_cursor_source_fidelity_errors(
    runner_path: Path,
    runner_source: str,
    outer_turns: RustItem | None,
) -> list[str]:
    """Bind the context-frozen finite Completion/Runtime/Ingress cursor."""

    errors: list[str] = []
    _require_exact_rust_tokens(
        runner_path,
        outer_turns,
        """
fn outer_ingress_turns(
    limit: usize,
    context_id: wire::HeightContextId,
    height: wire::Height,
) -> OuterIngressTurns {
    OuterIngressTurns::new(limit, context_id, height)
}
""",
        "ordinary ingress turn construction must delegate to the context-bound finite cursor",
        errors,
    )
    new_context = (("impl", "OuterIngressTurns"),)
    new_items = tuple(
        item
        for item in rust_items(runner_source, "new")
        if item.brace_context == new_context
    )
    if len(new_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one context-bound outer-ingress "
            f"cursor constructor; found {len(new_items)}"
        )
        new_item = None
    else:
        new_item = new_items[0]
        _require_rust_item_context(
            runner_path,
            new_item,
            new_context,
            "context-bound outer-ingress cursor constructor",
            errors,
        )
    _require_exact_rust_tokens(
        runner_path,
        new_item,
        """
fn new(limit: usize, context_id: wire::HeightContextId, height: wire::Height) -> Self {
    Self {
        context_id,
        height,
        cycles_remaining: limit.max(1),
        next_turn: OuterIngressTurn::Completion,
    }
}
""",
        "outer-ingress cursor must freeze its height context and start at Completion with a positive finite cycle bound",
        errors,
    )
    next_context = (("impl", "Iterator", "for", "OuterIngressTurns"),)
    next_items = tuple(
        item
        for item in rust_items(runner_source, "next")
        if item.brace_context == next_context
    )
    if len(next_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one outer-ingress cursor "
            f"Iterator::next implementation; found {len(next_items)}"
        )
        next_item = None
    else:
        next_item = next_items[0]
        _require_rust_item_context(
            runner_path,
            next_item,
            next_context,
            "finite outer-ingress cursor alternation",
            errors,
        )
    _require_exact_rust_tokens(
        runner_path,
        next_item,
        """
fn next(&mut self) -> Option<Self::Item> {
    if self.cycles_remaining == 0 {
        return None;
    }
    let turn = self.next_turn;
    self.next_turn = match turn {
        OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
        OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
        OuterIngressTurn::Ingress => {
            self.cycles_remaining -= 1;
            OuterIngressTurn::Completion
        }
    };
    Some(turn)
}
""",
        "ordinary ingress must alternate finite Completion, Runtime, and Ingress turns",
        errors,
    )
    rank_item = _require_rust_item(
        runner_path, runner_source, "outer_ingress_turn_index", errors
    )
    _require_exact_rust_tokens(
        runner_path,
        rank_item,
        """
const fn outer_ingress_turn_index(turn: OuterIngressTurn) -> u8 {
    match turn {
        OuterIngressTurn::Completion => 0,
        OuterIngressTurn::Runtime => 1,
        OuterIngressTurn::Ingress => 2,
    }
}
""",
        "outer-ingress cursor rank must use the same Completion, Runtime, and Ingress order",
        errors,
    )
    return errors


_REMOTE_PROPOSAL_REPLAY_ITEM_SHA256 = {
    "origin_new": "31a61793dfe490059e20eb77f75b9438f95386adc4a618c9eecc695eb9f3f4d7",
    "origin_rebind": "03b2c047f695feded9b1585a5f0bfa4cee5279f11c169b47bf2a9589631a9e9e",
    "origin_merge": "47cbedbb2fcd3da2122459143e65ed6196d14b7779e29d12d135d3fd7e7157f7",
    "origin_bind_fetch": "546e2a076aeccc4577fc4fcd31898a0cb645fcf2b086cf2a9ca825d7085c4a8b",
    "driver_default_bind": "8a0e749888af5c64b474d89b3048fcb1d91344776319e61c67415f4eeb894bcd",
    "driver_bind": "8117b5b9453be5494bc46f3ed9d164a1472411e6ed712067afcc554f32b6a1e3",
    "ownership_fetch_replay": "fec923e08dd4c0c33198a53369d808298c2a62d8a9abb2ef2aff6ac0c54ba182",
    "authority_mint": "ea8680ad00ae88245688355cc371724b4a133b112be43c3d46fe50f9cc4785e3",
    "authority_match": "fffb59ff81f0cdf5b0f8c1c222693c0fb586af021b2c68746ca5e846bf0ba8e5",
    "authority_pending": "cbea73229aa28cb8524d08e36bc0e89dabfe803f16791dfb1ea9519376082b82",
    "authority_fetch": "95992baebcab3fd7ecce16e4530609d27197b10e58b222764d0abc3e3acc3aae",
    "runtime_regression": "46c5d09beb1f80e9d5c7b2c83a8bf2d8dbfefa56705d915a8899f928a177d398",
}


def _remote_proposal_replay_source_fidelity_errors(
    repo_root: Path,
    runtime_path: Path,
    runtime_source: str,
) -> list[str]:
    """Bind authenticated remote Proposal ingress to its one ordinary Fetch."""

    errors: list[str] = []
    authority_path = runtime_path.with_name("v2_lifecycle_replay_authority.rs")
    if not authority_path.is_file() or authority_path.is_symlink():
        return [f"{authority_path}: remote Proposal replay authority source is required"]
    _path, authority_source = _read_reviewed_rust_source(
        repo_root,
        authority_path.relative_to(repo_root).as_posix(),
        errors,
        "remote Proposal replay authority source",
    )

    def one_item(
        path: Path,
        source: str,
        name: str,
        context: tuple[tuple[str, ...], ...],
        digest_key: str,
        description: str,
        *,
        attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        matches = tuple(
            item for item in rust_items(source, name) if item.brace_context == context
        )
        if len(matches) != 1:
            errors.append(f"{path}: require exactly one {description}; found {len(matches)}")
            return None
        item = matches[0]
        _require_rust_item_context(
            path,
            item,
            context,
            description,
            errors,
            expected_attributes=attributes,
        )
        _require_rust_item_token_sha256(
            path,
            item,
            _REMOTE_PROPOSAL_REPLAY_ITEM_SHA256[digest_key],
            description,
            errors,
        )
        return item

    def require_order(
        path: Path,
        item: RustItem | None,
        sequences: tuple[str, ...],
        diagnostic: str,
    ) -> None:
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
            errors.append(f"{path}:{item.line}: {diagnostic}")

    runtime_tokens = rust_code_tokens(runtime_source)
    for declaration, description in (
        (
            "remote_proposal_fetch_replay: Option<RemoteProposalFetchReplayEvidenceV1>",
            "effect ownership must retain exactly one opaque remote Proposal replay sidecar",
        ),
        (
            "remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "driver dispatch must carry exactly one opaque remote Proposal origin",
        ),
        (
            "deferred_remote_proposal_replay: BTreeMap<u128, AuthenticatedRemoteProposalDispatchOrigin>",
            "serialized runtime must retain one bounded deferred Proposal map",
        ),
        (
            "pending_remote_proposal_replay: Option<AuthenticatedRemoteProposalDispatchOrigin>",
            "serialized runtime must retain one immediate Proposal binding slot",
        ),
    ):
        count = _token_sequence_count(runtime_tokens, rust_code_tokens(declaration))
        if count != 1:
            errors.append(f"{runtime_path}: {description}; found {count}")

    origin_context = (("impl", "AuthenticatedRemoteProposalDispatchOrigin"),)
    origin_new = one_item(
        runtime_path,
        runtime_source,
        "new",
        origin_context,
        "origin_new",
        "authenticated remote Proposal origin constructor",
    )
    origin_rebind = one_item(
        runtime_path,
        runtime_source,
        "rebind_retained_ingress",
        origin_context,
        "origin_rebind",
        "authenticated remote Proposal retained-ingress rebind",
    )
    origin_merge = one_item(
        runtime_path,
        runtime_source,
        "merge_retained",
        origin_context,
        "origin_merge",
        "authenticated remote Proposal deferred merge",
    )
    origin_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_exact_fetch",
        origin_context,
        "origin_bind_fetch",
        "authenticated remote Proposal Fetch mint",
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_new,
        """
if !matches!(authenticated.payload(), wire::ConsensusMessageV2Payload::Proposal(_))
    || !ingress.exactly_matches_authenticated(&authenticated)
{
    return None;
}
Some(Self {
    authenticated,
    ingress,
})
""",
        "remote Proposal origin must bind one authenticated Proposal to its exact frozen ingress",
        errors,
    )
    require_order(
        runtime_path,
        origin_rebind,
        (
            "retained.exactly_matches_authenticated(&self.authenticated)",
            "let mut merged = self.ingress.clone();",
            "merged.merge_downstream(retained.clone()).ok()?;",
            "if merged != retained",
            "ingress: retained",
        ),
        "remote Proposal deferred rebase must preserve the same envelope and exact retained ingress",
    )
    require_order(
        runtime_path,
        origin_merge,
        (
            "self.authenticated.same_wire_envelope(&incumbent.authenticated)",
            "incumbent.rebind_retained_ingress(retained.clone())",
            "self.rebind_retained_ingress(retained)",
        ),
        "remote Proposal deferred merge must preserve one envelope and retained ingress",
    )
    _require_rust_token_sequence(
        runtime_path,
        origin_bind,
        """
RemoteProposalFetchReplayEvidenceV1::from_exact_authenticated_proposal(
    RemoteProposalReplayMintPermit::new(),
    self.authenticated,
    self.ingress,
    effect,
    pending,
)
""",
        "remote Proposal Fetch mint must consume the private one-shot permit and complete origin",
        errors,
    )

    trait_context = (("pub", "(", "crate", ")", "trait", "RuntimeDriver"),)
    default_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_remote_proposal_fetch_replay",
        trait_context,
        "driver_default_bind",
        "closed synthetic remote Proposal replay binder",
    )
    _require_rust_token_sequence(
        runtime_path,
        default_bind,
        "Result<(), ()> { Err(()) }",
        "synthetic runtime drivers must not mint remote Proposal replay authority",
        errors,
    )
    production_context = (("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),)
    production_bind = one_item(
        runtime_path,
        runtime_source,
        "bind_remote_proposal_fetch_replay",
        production_context,
        "driver_bind",
        "production remote Proposal replay binder",
    )
    require_order(
        runtime_path,
        production_bind,
        (
            "if effects.len() != ownership.len()",
            "AdapterEffect::FetchBody { certificate: None, .. }",
            "let Some((index, effect)) = fetches.next() else",
            "if fetches.next().is_some() || ownership[index].remote_proposal_fetch_replay.is_some()",
            "ownership[index].pending_adapter_effect_binding(effect)",
            "origin.bind_exact_fetch(effect, pending)",
            "ownership[index].remote_proposal_fetch_replay = Some(replay);",
        ),
        "ordinary Proposal replay must bind at most one exact uncertified Fetch",
    )
    dispatch = next(
        (
            item
            for item in rust_items(runtime_source, "dispatch")
            if item.brace_context == production_context
        ),
        None,
    )
    require_order(
        runtime_path,
        dispatch,
        (
            "if authenticated != tagged.ingress_ownership.is_some()",
            "let remote_proposal_replay = match (&tagged.command, &tagged.ingress_ownership)",
            "wire::ConsensusMessageV2Payload::Proposal(_)",
            "AuthenticatedRemoteProposalDispatchOrigin::new(",
            "message.clone()",
            "ingress.clone()",
            "self.bind_selected_producer_lifecycle(",
            "self.clear_selected_producer_lifecycle();",
            "let outcome = outcome?;",
            "remote_proposal_replay,",
        ),
        "authenticated Proposal dispatch must derive and transfer one exact replay origin",
    )

    generic_context = (
        ("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),
    )
    generic_items: dict[str, RustItem | None] = {}
    for name in (
        "reconcile_deferred_runtime_ownership_after_retirement",
        "accept_driver_dispatch",
        "dispatch_one_adapter_deferred",
    ):
        matches = tuple(
            item
            for item in rust_items(runtime_source, name)
            if item.brace_context == generic_context
        )
        if len(matches) != 1:
            errors.append(f"{runtime_path}: require exactly one remote Proposal replay {name}; found {len(matches)}")
            generic_items[name] = None
        else:
            generic_items[name] = matches[0]
    require_order(
        runtime_path,
        generic_items["reconcile_deferred_runtime_ownership_after_retirement"],
        (
            "let active = self.driver.all_deferred_admission_ordinals();",
            "let authenticated = self.driver.authenticated_deferred_admission_ordinals();",
            "self.deferred_remote_proposal_replay.retain(|ordinal, _| authenticated.contains(ordinal));",
            "self.deferred_remote_proposal_replay.iter().any(|(ordinal, origin)|",
            "ingress.get(ordinal) != Some(&origin.ingress)",
            "origin.ingress.exactly_matches_authenticated(&origin.authenticated)",
            "self.deferred_lifecycle_ownership = lifecycle;",
            "self.deferred_ingress_ownership = ingress;",
        ),
        "retirement reconciliation must prune and validate deferred Proposal replay",
    )
    require_order(
        runtime_path,
        generic_items["accept_driver_dispatch"],
        (
            "if self.pending_remote_proposal_replay.is_some()",
            "self.reconcile_deferred_ingress_ownership(deferred_ingress)",
            "self.deferred_remote_proposal_replay.retain(|ordinal, _| active.contains(ordinal));",
            "match (retry_unadmitted, deferred_ordinal, remote_proposal_replay)",
            "origin.merge_retained(incumbent, retained_ingress)",
            "origin.rebind_retained_ingress(retained_ingress)",
            "self.deferred_remote_proposal_replay.insert(ordinal, origin);",
            "self.pending_remote_proposal_replay = Some(origin);",
            "self.deferred_remote_proposal_replay.iter().any(|(ordinal, origin)|",
            "origin.ingress.exactly_matches_authenticated(&origin.authenticated)",
            "self.deferred_lifecycle_ownership = retained;",
        ),
        "driver acceptance must retain Proposal replay with its exact deferred ingress owner",
    )
    require_order(
        runtime_path,
        generic_items["dispatch_one_adapter_deferred"],
        (
            "self.deferred_ingress_ownership.remove(&evidence.admission_ordinal)",
            "self.deferred_remote_proposal_replay.remove(&evidence.admission_ordinal)",
            "(DeferredEventKind::ProposalReceived, Some(origin), Some(ingress))",
            "origin.rebind_retained_ingress(ingress.clone())",
            "self.retain_scheduler_ownership(",
            "if self.pending_remote_proposal_replay.is_some()",
            "self.pending_remote_proposal_replay = remote_proposal_replay;",
            "self.retain_effect_ownership(",
        ),
        "deferred Proposal replay must rebind the selected ProposalReceived ingress before effect ownership",
    )
    retain = next(
        (
            item
            for item in rust_items(runtime_source, "retain_effect_ownership")
            if item.brace_context == generic_context
        ),
        None,
    )
    if retain is None:
        errors.append(f"{runtime_path}: require exactly one runtime effect ownership retainer")
    else:
        _require_rust_token_sequence(
            runtime_path,
            retain,
            """
if effects.is_empty() {
    if let Some(origin) = self.pending_remote_proposal_replay.take() {
        self.driver.bind_remote_proposal_fetch_replay(origin, effects, &mut [])
""",
            "effect ownership must consume an empty pending Proposal replay through the closed binder",
            errors,
        )
        require_order(
            runtime_path,
            retain,
            (
                "let mut ownership = Vec::with_capacity(effects.len());",
                """
if let Some(origin) = self.pending_remote_proposal_replay.take() {
    self.driver.bind_remote_proposal_fetch_replay(origin, effects, &mut ownership)
""",
                "self.pending_effect_ownership = Some(ownership);",
            ),
            "effect ownership must consume the pending Proposal replay before batch publication",
        )

    ownership_context = (("impl", "RuntimeEffectOwnership"),)
    accessor = one_item(
        runtime_path,
        runtime_source,
        "exact_remote_proposal_fetch_replay",
        ownership_context,
        "ownership_fetch_replay",
        "exact remote Proposal Fetch replay accessor",
    )
    _require_rust_token_sequence(
        runtime_path,
        accessor,
        """
let replay = self.remote_proposal_fetch_replay.as_ref()?;
let pending = self.pending_adapter_effect_binding(effect)?;
replay.exactly_matches_fetch_pending(effect, &pending).then(|| replay.clone())
""",
        "remote Proposal replay accessor must require the exact Fetch pending binding",
        errors,
    )

    authority_context = (("impl", "RemoteProposalFetchReplayEvidenceV1"),)
    authority_mint = one_item(
        authority_path,
        authority_source,
        "from_exact_authenticated_proposal",
        authority_context,
        "authority_mint",
        "remote Proposal Fetch replay authority mint",
    )
    authority_match = one_item(
        authority_path,
        authority_source,
        "exactly_matches_fetch",
        authority_context,
        "authority_match",
        "remote Proposal Fetch replay matcher",
    )
    authority_pending = one_item(
        authority_path,
        authority_source,
        "exactly_matches_fetch_pending",
        authority_context,
        "authority_pending",
        "remote Proposal Fetch pending-binding matcher",
    )
    authority_fetch = one_item(
        authority_path,
        authority_source,
        "exact_remote_proposal_fetch",
        (),
        "authority_fetch",
        "exact ordinary remote Proposal Fetch classifier",
    )
    require_order(
        authority_path,
        authority_mint,
        (
            "let proposal = exact_remote_proposal_fetch(&authenticated, &ingress, effect)?;",
            "if !pending.exactly_binds_adapter_effect(effect)",
            "canonical_replay_authority(",
            "fetch_pending: Arc::new(pending)",
            "evidence.exactly_matches_fetch(effect).then_some(evidence)",
        ),
        "remote Proposal Fetch replay mint must validate the envelope, pending owner, and canonical authority",
    )
    _require_rust_token_sequence(
        authority_path,
        authority_pending,
        "self.exactly_matches_fetch(effect) && self.fetch_pending.as_ref() == pending",
        "remote Proposal Fetch pending matcher must retain exact effect and pending ownership",
        errors,
    )
    _require_rust_token_sequence(
        authority_path,
        authority_fetch,
        """
ingress.exactly_matches_authenticated(authenticated)
    && certified_sources.is_empty()
    && *round == proposal.round
    && *subject == proposal.subject
    && manifest == &proposal.manifest
    && tag.height() == round.height
    && tag.view() >= round.view
""",
        "remote Proposal Fetch replay must reject certified, foreign-coordinate, or foreign-manifest work",
        errors,
    )
    _require_rust_token_sequence(
        authority_path,
        authority_match,
        "self.fetch_pending.exactly_binds_adapter_effect(effect)",
        "remote Proposal Fetch matcher must retain the exact pending effect owner",
        errors,
    )

    test_context = (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),)
    regression = one_item(
        runtime_path,
        runtime_source,
        "authenticated_remote_proposal_retains_exact_fetch_store_validate_replay_origin",
        test_context,
        "runtime_regression",
        "authenticated remote Proposal Fetch/Store/Validate replay regression",
        attributes=("#[test]", "#[allow(clippy::too_many_lines)]"),
    )
    for sequence, diagnostic in (
        (
            "runtime.enqueue_network(wrong_signature).is_err()",
            "remote Proposal replay regression must reject a substituted signature",
        ),
        (
            "fetch_ownership.exact_remote_proposal_fetch_replay(&foreign_manifest_fetch).is_none()",
            "remote Proposal replay regression must reject a foreign manifest",
        ),
        (
            "fetch_ownership.exact_remote_proposal_fetch_replay(&certified_fetch).is_none()",
            "remote Proposal replay regression must reject a certified Fetch",
        ),
        (
            "fetch_replay.exactly_projects_store(store_effect, &store_pending)",
            "remote Proposal replay regression must project exact Fetch into Store",
        ),
    ):
        _require_rust_token_sequence(runtime_path, regression, sequence, diagnostic, errors)

    return errors


def _exact_serve_runtime_episode_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the finite exact-Serve predecessor episode in production Rust.

    The formal ingress rank relies on one immutable ticket ordinal, one
    selected strictly older lifecycle owner per bounded turn, and a mandatory
    full owner-set recheck before the ticket can enter target-only service.
    This source contract keeps those facts distinct from the trusted host
    scheduling/termination boundary.
    """

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    worker_path = base / "v2_worker.rs"
    runner_path = base / "v2_runner.rs"
    effects_path = base / "v2_effects.rs"
    runtime_path = base / "v2_runtime.rs"
    errors: list[str] = []
    for path, description in (
        (worker_path, "exact-Serve queue/service implementation"),
        (runner_path, "exact-Serve serialized-runner implementation"),
        (effects_path, "exact-Serve executor owner publication"),
        (runtime_path, "exact-Serve complete runtime owner comparison"),
    ):
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: {description} must be a regular file")
    if errors:
        return errors

    _loaded_path, worker_source = _read_reviewed_rust_source(
        repo_root,
        worker_path.relative_to(repo_root).as_posix(),
        errors,
        "exact-Serve queue/service implementation",
    )
    _loaded_path, runner_source = _read_reviewed_rust_source(
        repo_root,
        runner_path.relative_to(repo_root).as_posix(),
        errors,
        "exact-Serve serialized-runner implementation",
    )
    effects_source = effects_path.read_text(encoding="utf-8")
    _loaded_path, runtime_source = _read_reviewed_rust_source(
        repo_root,
        runtime_path.relative_to(repo_root).as_posix(),
        errors,
        "exact-Serve complete runtime owner comparison",
    )

    struct_items: dict[str, RustItem | None] = {}
    for struct_name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256.items()
    ):
        matches = rust_struct_items(worker_source, struct_name)
        if len(matches) != 1:
            errors.append(
                f"{worker_path}: require exactly one real exact-Serve state "
                f"carrier named {struct_name}; found {len(matches)}"
            )
            continue
        item = matches[0]
        struct_items[struct_name] = item
        _require_rust_item_context(
            worker_path,
            item,
            (),
            f"exact-Serve state carrier {struct_name}",
            errors,
            expected_attributes={
                "V2IoCertifiedServeIngressReservation": ("#[derive(Debug)]",),
                "V2IoCompletionOwnership": ("#[derive(Clone, Copy, Debug)]",),
                "CertifiedServeProducerEpisode": ("#[must_use]",),
                "V2IoCommandQueueState": (),
            }[struct_name],
        )
        _require_rust_item_token_sha256(
            worker_path,
            item,
            expected_sha256,
            f"exact-Serve state carrier {struct_name}",
            errors,
        )

    witness_struct_items: dict[str, RustItem | None] = {}
    for struct_name, expected_sha256 in (
        _EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256.items()
    ):
        matches = rust_struct_items(runtime_source, struct_name)
        if len(matches) != 1:
            errors.append(
                f"{runtime_path}: require exactly one process-local exact-Serve "
                f"predecessor witness named {struct_name}; found {len(matches)}"
            )
            continue
        item = matches[0]
        witness_struct_items[struct_name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            (),
            f"exact-Serve predecessor witness {struct_name}",
            errors,
            expected_attributes=(
                "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]",
            ),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"exact-Serve predecessor witness {struct_name}",
            errors,
        )

    _require_rust_token_sequence(
        runtime_path,
        witness_struct_items.get("ExactServePredecessorCompletionEvidence"),
        """
pub(crate) struct ExactServePredecessorCompletionEvidence {
    lifecycle_ordinal: u128,
    lifecycle_ordinal_complement: u128,
}
""",
        "process-local completion evidence must bind one immutable ordinal and "
        "its exact integrity complement without a wire carrier",
        errors,
    )

    _require_rust_token_sequence(
        runtime_path,
        witness_struct_items.get("ExactServePredecessorEpisodeWitness"),
        """
pub(crate) struct ExactServePredecessorEpisodeWitness {
    serve_lifecycle_ordinal: u128,
    predecessor_lifecycle_ordinal: u128,
    episode: u128,
}
""",
        "the process-local exact-Serve witness must bind the immutable target, "
        "strict predecessor, and monotone episode without a wire carrier",
        errors,
    )

    reservation_state = struct_items.get("V2IoCertifiedServeIngressReservation")
    _require_rust_token_sequence(
        worker_path,
        struct_items.get("V2IoCompletionOwnership"),
        """
struct V2IoCompletionOwnership {
    retained_at: Instant,
    service_debt: u64,
    requires_runtime_capacity: bool,
    runtime_lifecycle_ordinal: Option<u128>,
}
""",
        "completion ownership must retain time/debt, runtime-capacity class, and "
        "the exact optional shared lifecycle ordinal in one copy-only carrier",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        reservation_state,
        """
struct V2IoCertifiedServeIngressReservation {
    id: CertifiedServeIngressReservationId,
    lifecycle_id: CertifiedServeLifecycleId,
    projection: CertifiedServeIngressProjection,
    request: wire::CertifiedBodyRequest,
    state: CertifiedServeIngressReservationState,
    handed_off: Option<Arc<AtomicBool>>,
    carrier_ordinal: Option<u64>,
    runtime_episode: CertifiedServeRuntimeEpisodeState,
    last_predecessor_episode_witness: Option<ExactServePredecessorEpisodeWitness>,
}
""",
        "the exact-Serve reservation must keep its physical scheduler ticket, "
        "logical lifecycle, payload, carrier, bounded runtime turn, and last "
        "consumed predecessor witness in their distinct reviewed field roles",
        errors,
    )
    queue_state = struct_items.get("V2IoCommandQueueState")
    _require_rust_token_sequence(
        worker_path,
        queue_state,
        """
producer_episode_due: bool,
producer_episode_active: bool,
sender_open: bool,
receiver_open: bool,
""",
        "the queue state must retain distinct one-shot due and finite active "
        "producer-episode fields before endpoint liveness",
        errors,
    )

    _require_rust_source_token_sequence(
        worker_path,
        worker_source,
        """
enum CertifiedServeRuntimeEpisodeState {
    Ready,
    Claimed {
        predecessor_ordinal: Option<u128>,
    },
    Complete,
}
        """,
        "exact-Serve episode must retain distinct ready, one-owner claimed, "
        "and sealed complete states; only a new runtime witness may reopen complete",
        errors,
    )

    restore_item = _require_rust_item(
        worker_path,
        worker_source,
        "restore_certified_serve_tombstones",
        errors,
    )
    _require_rust_item_context(
        worker_path,
        restore_item,
        (),
        "restart-restored exact-Serve predecessor witness initialization",
        errors,
        expected_attributes=(
            "#[allow(clippy::too_many_arguments, clippy::type_complexity)]",
        ),
    )
    _require_rust_item_token_sha256(
        worker_path,
        restore_item,
        _EXACT_SERVE_RUNTIME_EPISODE_RESTORE_ITEM_SHA256[
            "restore_certified_serve_tombstones"
        ],
        "restart-restored exact-Serve predecessor witness initialization",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        restore_item,
        """
handed_off: None,
carrier_ordinal: None,
runtime_episode: CertifiedServeRuntimeEpisodeState::Ready,
last_predecessor_episode_witness: None,
""",
        "restart-restored reservations must begin Ready with no live carrier and no synthetic consumed predecessor witness",
        errors,
    )

    reservation_items: dict[str, RustItem | None] = {}
    reservation_context = (("impl", "V2IoCertifiedServeIngressReservation"),)
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256.items()
    ):
        matching = [
            item
            for item in rust_items(worker_source, name)
            if item.brace_context == reservation_context
        ]
        item = matching[0] if len(matching) == 1 else None
        if item is None:
            errors.append(
                f"{worker_path}: require exactly one real exact-Serve ingress "
                f"reservation method named {name}; found {len(matching)}"
            )
        reservation_items[name] = item
        _require_rust_item_context(
            worker_path,
            item,
            reservation_context,
            f"exact-Serve ingress reservation identity seam {name}",
            errors,
        )
        _require_rust_item_token_sha256(
            worker_path,
            item,
            expected_sha256,
            f"exact-Serve ingress reservation identity seam {name}",
            errors,
        )

    reservation_barrier = reservation_items.get("barrier")
    for sequence, description in (
        (
            "if self.handed_off.is_none()",
            "an exact-Serve barrier must retain its live fair-ingress carrier",
        ),
        (
            "let carrier_ordinal = self.carrier_ordinal.ok_or_else",
            "an exact-Serve barrier must retain its fresh physical carrier ordinal",
        ),
        (
            "if self.id.0 == 0 || carrier_ordinal == 0",
            "logical and physical exact-Serve ordinals must both remain nonzero",
        ),
        (
            """
CertifiedServeBarrier {
    request_hash: self.projection.request_hash,
    scheduler_ordinal: self.id.0,
    lifecycle_id: self.lifecycle_id,
    carrier_ordinal,
}
""",
            "the runner barrier must project the exact request, physical scheduler "
            "ticket, logical lifecycle, and carrier",
        ),
    ):
        _require_rust_token_sequence(
            worker_path,
            reservation_barrier,
            sequence,
            description,
            errors,
        )

    reservation_match = reservation_items.get("matches_barrier")
    for sequence, description in (
        (
            "self.id.0 == barrier.scheduler_ordinal",
            "episode claims must retain the exact physical Serve scheduler ticket",
        ),
        (
            "self.lifecycle_id == barrier.lifecycle_id",
            "episode claims must retain the exact logical Serve lifecycle",
        ),
        (
            "self.projection.request_hash == barrier.request_hash",
            "episode claims must retain the exact Serve request hash",
        ),
        (
            "self.carrier_ordinal == Some(barrier.carrier_ordinal)",
            "episode claims must retain the selected physical carrier occurrence",
        ),
        (
            "self.handed_off.is_some()",
            "episode claims must retain the live fair-ingress handoff",
        ),
    ):
        _require_rust_token_sequence(
            worker_path,
            reservation_match,
            sequence,
            description,
            errors,
        )

    worker_items: dict[str, RustItem | None] = {}
    channel_builder = _require_rust_item(
        worker_path,
        worker_source,
        "build_v2_io_command_channel",
        errors,
    )
    _require_rust_item_context(
        worker_path,
        channel_builder,
        (),
        "exact-Serve command-channel initialization",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments)]",),
    )
    worker_items["build_v2_io_command_channel"] = channel_builder
    for owner, name, description in (
        (
            "V2IoCommandQueue",
            "close_receiver",
            "receiver teardown producer-episode retirement",
        ),
        (
            "V2IoCommandQueue",
            "reserve_serve_ingress",
            "immutable exact-Serve ticket admission",
        ),
        (
            "V2IoCommandQueue",
            "retire_selected_serve_ingress_occurrence",
            "atomic final-Serve to producer-episode handoff",
        ),
        (
            "V2IoCommandQueue",
            "try_begin_producer_episode",
            "queue-atomic ordinary producer exclusion",
        ),
        (
            "V2IoCommandQueue",
            "suspend_materialized_serve_barrier_for_runtime_predecessor",
            "physical target-unit transfer to one older owner",
        ),
        (
            "V2IoCommandQueue",
            "serve_barrier",
            "exact-Serve runner barrier projection",
        ),
        (
            "V2IoCommandQueue",
            "claim_serve_runtime_episode",
            "one-turn exact-Serve episode claim",
        ),
        (
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "exact-Serve predecessor witness consumption and bounded reopening",
        ),
        (
            "V2IoCommandQueue",
            "serve_runtime_predecessor_capacity_available",
            "exact-Serve predecessor capacity preflight",
        ),
        (
            "V2IoCommandQueue",
            "finish_serve_runtime_episode_turn",
            "mandatory exact-Serve post-turn settlement",
        ),
        (
            "V2IoCommandQueue",
            "try_send_as",
            "strict older-owner I/O admission",
        ),
        (
            "ProductionV2Services",
            "certified_serve_barrier",
            "production exact-Serve barrier projection",
        ),
        (
            "ProductionV2Services",
            "claim_certified_serve_runtime_episode",
            "production exact-Serve episode claim",
        ),
        (
            "ProductionV2Services",
            "certified_serve_predecessor_completion_evidence",
            "non-consuming exact-Serve completion evidence projection",
        ),
        (
            "ProductionV2Services",
            "observe_certified_serve_predecessor_episode_witness",
            "production exact-Serve predecessor witness forwarding",
        ),
        (
            "ProductionV2Services",
            "certified_serve_runtime_predecessor_capacity_available",
            "production exact-Serve predecessor capacity preflight",
        ),
        (
            "ProductionV2Services",
            "finish_certified_serve_runtime_episode_turn",
            "production exact-Serve post-turn settlement",
        ),
        (
            "ProductionV2Services",
            "try_begin_certified_serve_producer_episode",
            "production ordinary producer exclusion",
        ),
        (
            "ProductionV2Services",
            "take_exact_serve_predecessor_completion",
            "strict older completed-owner selection",
        ),
        (
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "shared strict/inclusive completed-owner selection",
        ),
        (
            "ProductionV2Services",
            "drain_exact_serve_runtime_predecessor",
            "one-completion exact-Serve predecessor drain",
        ),
        (
            "ProductionV2Services",
            "drain_completions_inner",
            "policy-bounded completion drain",
        ),
    ):
        key = f"{owner}::{name}"
        worker_items[key] = _require_qualified_rust_item(
            worker_path,
            worker_source,
            owner,
            name,
            errors,
            description,
        )

    drop_matches = [
        item
        for item in rust_items(worker_source, "drop")
        if item.brace_context
        == (("impl", "Drop", "for", "CertifiedServeProducerEpisode"),)
    ]
    producer_drop = drop_matches[0] if len(drop_matches) == 1 else None
    if producer_drop is None:
        errors.append(
            f"{worker_path}: require exactly one real "
            "CertifiedServeProducerEpisode::drop item; "
            f"found {len(drop_matches)}"
        )
    else:
        _require_rust_item_context(
            worker_path,
            producer_drop,
            (("impl", "Drop", "for", "CertifiedServeProducerEpisode"),),
            "finite ordinary producer-episode retirement",
            errors,
        )
    worker_items["CertifiedServeProducerEpisode::drop"] = producer_drop

    for key, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256.items()
    ):
        _require_rust_item_token_sha256(
            worker_path,
            worker_items.get(key),
            expected_sha256,
            f"exact-Serve production seam {key}",
            errors,
        )

    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::certified_serve_predecessor_completion_evidence"
        ),
        """
pub(crate) fn certified_serve_predecessor_completion_evidence(
    &self,
    runtime_capacity_available: bool,
    serve_lifecycle_ordinal: u128,
) -> Result<Option<ExactServePredecessorCompletionEvidence>, String> {
    if serve_lifecycle_ordinal == 0 {
        return Err("Sumeragi v2 Serve completion cut was zero".to_owned());
    }
    let ownership_position =
        usize::from(!runtime_capacity_available && self.held_io_completion.is_some());
    let io_ordinal = self
        .io
        .as_ref()
        .and_then(|io| io.completion_ownership_at(ownership_position))
        .filter(|owned| runtime_capacity_available || !owned.requires_runtime_capacity)
        .and_then(|owned| owned.runtime_lifecycle_ordinal);
    if io_ordinal == Some(0) {
        return Err("Sumeragi v2 I/O completion retained a zero lifecycle ordinal".to_owned());
    }
    let io_ordinal = io_ordinal.filter(|ordinal| *ordinal < serve_lifecycle_ordinal);

    let mut local_ordinal = None;
    if runtime_capacity_available {
        for completion in &self.local_completions {
            let ordinal = completion.runtime_lifecycle_ordinal();
            if ordinal == 0 {
                return Err(
                    "Sumeragi v2 local completion retained a zero lifecycle ordinal".to_owned(),
                );
            }
            if ordinal < serve_lifecycle_ordinal {
                local_ordinal =
                    Some(local_ordinal.map_or(ordinal, |current: u128| current.min(ordinal)));
            }
        }
    }
    let ordinal = match (io_ordinal, local_ordinal) {
        (Some(io), Some(local)) => Some(io.min(local)),
        (Some(io), None) => Some(io),
        (None, Some(local)) => Some(local),
        (None, None) => None,
    };
    ordinal
        .map(|ordinal| {
            ExactServePredecessorCompletionEvidence::try_new(ordinal)
                .ok_or_else(|| "Sumeragi v2 Serve completion evidence was invalid".to_owned())
        })
        .transpose()
}
""",
        "exact-Serve completion projection must be non-consuming, capacity-gated, "
        "strictly older, least-ordinal, and fail closed on invalid ownership",
        errors,
    )

    completion_evidence_production_callers: list[tuple[Path, str, int]] = []
    completion_constructor_tokens = rust_code_tokens(
        "ExactServePredecessorCompletionEvidence::try_new("
    )
    core_source_root = repo_root / "crates" / "iroha_core" / "src"
    for source_path in sorted(core_source_root.rglob("*.rs")):
        if not source_path.is_file() or source_path.is_symlink():
            errors.append(
                f"{source_path}: completion-evidence caller inventory requires regular Rust files"
            )
            continue
        source = source_path.read_text(encoding="utf-8")
        for item in _rust_all_function_items(
            source,
            references=("ExactServePredecessorCompletionEvidence::try_new",),
        ):
            if _rust_item_is_test_only(item):
                continue
            constructor_count = _token_sequence_count(
                rust_code_tokens(item.body),
                completion_constructor_tokens,
            )
            if constructor_count:
                completion_evidence_production_callers.append(
                    (source_path, item.name, constructor_count)
                )
    expected_completion_evidence_callers = [
        (
            worker_path,
            "certified_serve_predecessor_completion_evidence",
            1,
        )
    ]
    if completion_evidence_production_callers != expected_completion_evidence_callers:
        errors.append(
            "exact-Serve completion evidence must be minted exactly once and only "
            "by the reviewed non-consuming ProductionV2Services projection; found "
            f"{completion_evidence_production_callers!r}"
        )

    completion_provenance_items: dict[str, RustItem | None] = {}
    for qualified_name, expected_sha256 in (
        _EXACT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256.items()
    ):
        if "::" in qualified_name:
            owner, name = qualified_name.rsplit("::", 1)
            item = _require_qualified_rust_item(
                worker_path,
                worker_source,
                owner,
                name,
                errors,
                f"exact-Serve completion provenance seam {qualified_name}",
            )
        else:
            item = _require_rust_item(
                worker_path,
                worker_source,
                qualified_name,
                errors,
            )
            _require_rust_item_context(
                worker_path,
                item,
                (),
                f"exact-Serve completion provenance seam {qualified_name}",
                errors,
            )
        completion_provenance_items[qualified_name] = item
        _require_rust_item_token_sha256(
            worker_path,
            item,
            expected_sha256,
            f"exact-Serve completion provenance seam {qualified_name}",
            errors,
        )

    for key, expected_source, description in (
        (
            "V2IoCommand::runtime_lifecycle_ordinal",
            """
const fn runtime_lifecycle_ordinal(&self) -> Option<u128> {
    match self {
        Self::Sign { task, .. } => Some(task.lifecycle_ordinal()),
        Self::Store(task) => Some(task.lifecycle_ordinal()),
        Self::Validate(task) => Some(task.lifecycle_ordinal()),
        Self::Apply(task) => Some(task.lifecycle_ordinal()),
        Self::Serve { .. } | Self::LoadCandidate { .. } | Self::Retire(_) | Self::Shutdown => {
            None
        }
    }
}
""",
            "every completion-producing I/O command must project its immutable "
            "runtime lifecycle ordinal while non-runtime commands project none",
        ),
        (
            "V2IoAdmission::retain_completion",
            """
fn retain_completion(
    &self,
    retained_at: Instant,
    requires_runtime_capacity: bool,
    runtime_lifecycle_ordinal: Option<u128>,
) {
    let mut state = self
        .completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        state.owned.len() < self.completion_capacity,
        "Sumeragi v2 I/O worker exceeded bounded completion ownership"
    );
    state.owned.push_back(V2IoCompletionOwnership {
        retained_at,
        service_debt: 0,
        requires_runtime_capacity,
        runtime_lifecycle_ordinal,
    });
}
""",
            "completion publication must atomically retain the exact capacity "
            "class and lifecycle ordinal at the bounded ownership tail",
        ),
        (
            "V2IoAdmission::abandon_latest_completion",
            """
fn abandon_latest_completion(&self) {
    let mut state = self
        .completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state
        .owned
        .pop_back()
        .expect("failed completion send must retain its ownership record");
}
""",
            "a failed send must abandon only the just-retained completion tail",
        ),
        (
            "V2IoAdmission::completion_ownership_at",
            """
fn completion_ownership_at(&self, position: usize) -> Option<V2IoCompletionOwnership> {
    self.completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .owned
        .get(position)
        .copied()
}
""",
            "completion ownership projection must copy the exact indexed record without consuming it",
        ),
        (
            "V2IoHandle::completion_ownership_at",
            """
fn completion_ownership_at(&self, position: usize) -> Option<V2IoCompletionOwnership> {
    self.admission.completion_ownership_at(position)
}
""",
            "the I/O handle must delegate the exact non-consuming ownership position",
        ),
        (
            "LocalCompletion::runtime_lifecycle_ordinal",
            """
const fn runtime_lifecycle_ordinal(&self) -> u128 {
    match self {
        Self::Reconstructed { task, .. } => task.lifecycle_ordinal(),
    }
}
""",
            "every local completion must project the immutable lifecycle ordinal "
            "of its original runtime task",
        ),
        (
            "send_completion_with_lifecycle_ordinal",
            """
fn send_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: Result<V2IoCompletion, String>,
    runtime_lifecycle_ordinal: Option<u128>,
) {
    let completion = completion.unwrap_or_else(V2IoCompletion::Failed);
    let _ = send_tracked_completion_with_lifecycle_ordinal(
        sender,
        admission,
        completion,
        runtime_lifecycle_ordinal,
    );
}
""",
            "the production completion wrapper must forward the captured runtime "
            "lifecycle ordinal unchanged to tracked publication",
        ),
        (
            "send_tracked_completion_with_lifecycle_ordinal",
            """
fn send_tracked_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
    runtime_lifecycle_ordinal: Option<u128>,
) -> Result<(), mpsc::SendError<V2IoCompletion>> {
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
    );
    sender.send(completion).inspect_err(|_| {
        admission.abandon_latest_completion();
    })
}
""",
            "blocking completion publication must retain exact ownership before send and abandon it on failure",
        ),
        (
            "try_send_tracked_completion_with_lifecycle_ordinal",
            """
fn try_send_tracked_completion_with_lifecycle_ordinal(
    sender: &mpsc::SyncSender<V2IoCompletion>,
    admission: &V2IoAdmission,
    completion: V2IoCompletion,
    runtime_lifecycle_ordinal: Option<u128>,
) -> Result<(), mpsc::TrySendError<V2IoCompletion>> {
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
    );
    sender.try_send(completion).inspect_err(|_| {
        admission.abandon_latest_completion();
    })
}
""",
            "nonblocking completion publication must retain exact ownership before send and abandon it on failure",
        ),
    ):
        _require_exact_rust_tokens(
            worker_path,
            completion_provenance_items.get(key),
            expected_source,
            description,
            errors,
        )

    completion_spawn = completion_provenance_items.get("V2IoHandle::spawn")
    _require_rust_token_sequence(
        worker_path,
        completion_spawn,
        """
let work_id = command.work_id();
let serve_lifecycle_id = command.serve_lifecycle_id();
let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal();
match command {
""",
        "the I/O worker must capture exact completion provenance before moving "
        "the command into execution",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        completion_spawn,
        """
send_completion_with_lifecycle_ordinal(
    &completion_tx,
    &worker_admission,
    Ok(completion),
    runtime_lifecycle_ordinal,
);
""",
        "the I/O worker must forward the pre-execution runtime lifecycle ordinal "
        "unchanged with the successful completion",
        errors,
    )
    if completion_spawn is not None:
        spawn_tokens = rust_code_tokens(completion_spawn.source)
        capture_positions = _token_sequence_positions(
            spawn_tokens,
            rust_code_tokens(
                "let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal();"
            ),
        )
        forward_positions = _token_sequence_positions(
            spawn_tokens,
            rust_code_tokens(
                """
send_completion_with_lifecycle_ordinal(
    &completion_tx,
    &worker_admission,
    Ok(completion),
    runtime_lifecycle_ordinal,
)
"""
            ),
        )
        if (
            len(capture_positions) != 1
            or len(forward_positions) != 1
            or capture_positions[0] >= forward_positions[0]
        ):
            errors.append(
                f"{worker_path}:{completion_spawn.line}: the I/O worker must "
                "capture exactly one command lifecycle ordinal before execution "
                "and forward it exactly once after successful completion"
            )

    _require_rust_token_sequence(
        worker_path,
        channel_builder,
        """
producer_episode_due: false,
producer_episode_active: false,
""",
        "the command channel initializer must clear producer-episode due "
        "immediately before active",
        errors,
    )
    close_receiver = worker_items.get("V2IoCommandQueue::close_receiver")
    _require_rust_token_sequence(
        worker_path,
        close_receiver,
        """
state.receiver_open = false;
state.producer_episode_due = false;
state.producer_episode_active = false;
self.rollback_serve_barrier(&mut state)
""",
        "receiver teardown must clear producer-episode due before active and "
        "Serve rollback",
        errors,
    )

    reserve = worker_items.get("V2IoCommandQueue::reserve_serve_ingress")
    _require_rust_token_sequence(
        worker_path,
        reserve,
        """
let ordinal = match self.lifecycle_ordinals.reserve_one() {
    Ok(ordinal) if ordinal > state.next_serve_ingress_reservation_ordinal => ordinal,
""",
        "exact-Serve tickets must use a fresh actor-global monotone ordinal",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        reserve,
        """
runtime_episode: CertifiedServeRuntimeEpisodeState::Ready,
last_predecessor_episode_witness: None,
""",
        "each fresh physical exact-Serve occurrence must start ready with no "
        "consumed predecessor witness",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        reserve,
        """
if state.producer_episode_due || state.producer_episode_active {
    return Err(CertifiedServeIngressReserveError::Busy);
}
""",
        "fresh Serve admission must not cross a due or active producer episode",
        errors,
    )

    retire = worker_items.get(
        "V2IoCommandQueue::retire_selected_serve_ingress_occurrence"
    )
    _require_rust_token_sequence(
        worker_path,
        retire,
        """
let promoted = Self::promote_next_serve_ingress_waiter(state);
if !promoted
    && state.serve_ingress_reservation.is_none()
    && state.serve_ingress_waiters.is_empty()
    && state.serve_barrier.is_none()
    && state.sender_open
    && state.receiver_open
{
    state.producer_episode_due = true;
}
promoted
""",
        "final frozen Serve retirement must atomically arm exactly one producer episode after every admitted waiter is exhausted",
        errors,
    )

    producer_start = worker_items.get(
        "V2IoCommandQueue::try_begin_producer_episode"
    )
    _require_rust_token_sequence(
        worker_path,
        producer_start,
        """
if state.serve_ingress_reservation.is_some()
    || !state.serve_ingress_waiters.is_empty()
    || state.serve_barrier.is_some()
{
    return Ok(None);
}
if state.producer_episode_active {
    return Err("Sumeragi v2 runner nested an I/O producer episode".to_owned());
}
state.producer_episode_due = false;
state.producer_episode_active = true;
""",
        "ordinary producers must consume the one-shot handoff and acquire one "
        "queue-atomic episode only when no exact target exists",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        producer_drop,
        """
let mut state = self.queue.lock();
if !state.producer_episode_active {
    return;
}
state.producer_episode_active = false;
""",
        "ordinary producer episodes must retire under the same queue lock",
        errors,
    )

    claim = worker_items.get("V2IoCommandQueue::claim_serve_runtime_episode")
    _require_rust_token_sequence(
        worker_path,
        claim,
        """
CertifiedServeRuntimeEpisodeState::Ready => {
    reservation.runtime_episode = CertifiedServeRuntimeEpisodeState::Claimed {
        predecessor_ordinal: None,
    };
    Ok(true)
}
CertifiedServeRuntimeEpisodeState::Claimed { .. }
| CertifiedServeRuntimeEpisodeState::Complete => Ok(false),
""",
        "one exact occurrence may claim only one unsettled predecessor turn",
        errors,
    )

    observe_witness = worker_items.get(
        "V2IoCommandQueue::observe_serve_predecessor_episode_witness"
    )
    for sequence, description in (
        (
            """
if !witness.validate_exact()
    || witness.serve_lifecycle_ordinal() != barrier.scheduler_ordinal()
    || witness.predecessor_lifecycle_ordinal() >= barrier.scheduler_ordinal()
{
    return Err("Sumeragi v2 Serve predecessor episode witness was invalid".to_owned());
}
""",
            "the worker must validate the exact target and strict predecessor before consuming a witness",
        ),
        (
            """
if witness.episode() < previous.episode() {
    return Err("Sumeragi v2 Serve predecessor episode regressed".to_owned());
}
if witness.episode() == previous.episode() {
    if witness != previous {
        return Err(
            "Sumeragi v2 Serve predecessor episode changed exact evidence".to_owned(),
        );
    }
    return Ok(false);
}
""",
            "a repeated witness must stutter while conflicting or regressing evidence fails closed",
        ),
        (
            """
let expected_episode = previous.episode().checked_add(1).ok_or_else(|| {
    "Sumeragi v2 Serve predecessor episode consumer ordinal overflowed".to_owned()
})?;
if witness.episode() != expected_episode {
    return Err("Sumeragi v2 Serve predecessor episode skipped an ordinal".to_owned());
}
""",
            "a replacement witness must advance by exactly one checked consumer episode",
        ),
        (
            """
} else if witness.episode() != 1 {
    return Err("Sumeragi v2 Serve predecessor episode did not start at one".to_owned());
}
reservation.last_predecessor_episode_witness = Some(witness);
""",
            "the first consumed predecessor witness must begin at one and become immutable reservation evidence",
        ),
        (
            """
if reservation.runtime_episode == CertifiedServeRuntimeEpisodeState::Complete {
    reservation.runtime_episode = CertifiedServeRuntimeEpisodeState::Ready;
    return Ok(true);
}
Ok(false)
""",
            "only a newly consumed witness may reopen a sealed Complete target to Ready",
        ),
    ):
        _require_rust_token_sequence(
            worker_path,
            observe_witness,
            sequence,
            description,
            errors,
        )

    capacity = worker_items.get(
        "V2IoCommandQueue::serve_runtime_predecessor_capacity_available"
    )
    _require_rust_token_sequence(
        worker_path,
        capacity,
        """
Ok(transferable_target_slot
    || (state.commands.len() < self.capacity
        && self.admission.has_capacity(V2IoAdmissionClass::Consensus)))
""",
        "an older causal owner may run only with a free consensus unit or the "
        "atomically transferable target unit",
        errors,
    )

    finish = worker_items.get(
        "V2IoCommandQueue::finish_serve_runtime_episode_turn"
    )
    _require_rust_token_sequence(
        worker_path,
        finish,
        """
reservation.runtime_episode = if older_predecessor_remains {
    CertifiedServeRuntimeEpisodeState::Ready
} else {
    CertifiedServeRuntimeEpisodeState::Complete
};
        """,
        "the mandatory full recheck must either reopen one bounded turn or "
        "seal the occurrence until a strictly newer runtime witness arrives",
        errors,
    )

    suspend = worker_items.get(
        "V2IoCommandQueue::suspend_materialized_serve_barrier_for_runtime_predecessor"
    )
    _require_rust_token_sequence(
        worker_path,
        suspend,
        """
assert_eq!(index + 1, state.commands.len(),
    "later I/O work cannot appear behind an uncommitted Serve barrier");
""",
        "only the physical FIFO-tail target unit may transfer to an older owner",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        suspend,
        """
.state = V2IoServeState::PendingCapacity;
assert!(state.pending_serve_requests.insert(lifecycle_id, request).is_none(),
    "materialized Serve barrier cannot already own a pending request");
self.admission.release();
""",
        "target-unit transfer must retain the logical request and dematerialize "
        "only its physical placeholder",
        errors,
    )

    serve_barrier = worker_items.get("V2IoCommandQueue::serve_barrier")
    _require_rust_token_sequence(
        worker_path,
        serve_barrier,
        """
if let Some(reservation) = ingress
    && (state
        .serve_by_request
        .get(&reservation.projection.request_hash)
        != Some(&reservation.lifecycle_id)
        || !state.serves.contains_key(&reservation.lifecycle_id))
{
    return Err("Sumeragi v2 Serve barrier lost its raw-admission lifecycle".to_owned());
}
""",
        "a raw exact-Serve barrier must remain indexed by its immutable logical lifecycle",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        serve_barrier,
        """
let barrier = reservation.barrier()?;
if barrier.request_hash != lifecycle_id.request_hash || barrier.lifecycle_id != lifecycle_id
{
    return Err("Sumeragi v2 Serve barrier changed its lifecycle request".to_owned());
}
""",
        "a materialized exact-Serve barrier must validate both request and logical lifecycle",
        errors,
    )

    enqueue = worker_items.get("V2IoCommandQueue::try_send_as")
    _require_rust_token_sequence(
        worker_path,
        enqueue,
        """
if command_ordinal >= reservation.id.0 {
    return None;
}
""",
        "equal or later causal work must not enter an exact ticket's frozen prefix",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        enqueue,
        """
CertifiedServeRuntimeEpisodeState::Claimed {
    predecessor_ordinal: Some(existing),
} if existing == command_ordinal => Some(command_ordinal),
""",
        "one claimed turn may admit fanout only for its already-selected owner",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        enqueue,
        """
let exact_target_active = state.serve_ingress_reservation.is_some()
    || !state.serve_ingress_waiters.is_empty()
    || state.serve_barrier.is_some();
if exact_target_active && exact_predecessor_ordinal.is_none() {
    return Err(V2IoTrySendError::Full(command));
}
""",
        "later causal, Control, Completion, and priority work must be blocked "
        "while the selected target, any admitted waiter, or its materialized "
        "barrier owns the next position",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        enqueue,
        """
if suspended_target {
    assert!(self.materialize_serve_barrier(&mut state),
        "failed predecessor admission must restore its exact Serve placeholder");
}
""",
        "failed older-owner admission must restore the exact target atomically",
        errors,
    )

    _require_exact_rust_tokens(
        worker_path,
        worker_items.get("ProductionV2Services::certified_serve_barrier"),
        """
pub(crate) fn certified_serve_barrier(&self) -> Result<Option<CertifiedServeBarrier>, String> {
    self.io.as_ref().map_or(Ok(None), V2IoHandle::serve_barrier)
}
""",
        "the production exact-Serve barrier wrapper must project only through the attached I/O owner",
        errors,
    )
    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::claim_certified_serve_runtime_episode"
        ),
        """
pub(crate) fn claim_certified_serve_runtime_episode(
    &self,
    barrier: CertifiedServeBarrier,
) -> Result<bool, String> {
    self.io
        .as_ref()
        .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?
        .claim_serve_runtime_episode(barrier)
}
""",
        "the production exact-Serve claim wrapper must fail closed and forward the exact barrier",
        errors,
    )
    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::observe_certified_serve_predecessor_episode_witness"
        ),
        """
pub(crate) fn observe_certified_serve_predecessor_episode_witness(
    &self,
    barrier: CertifiedServeBarrier,
    witness: ExactServePredecessorEpisodeWitness,
) -> Result<bool, String> {
    self.io
        .as_ref()
        .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?
        .observe_serve_predecessor_episode_witness(barrier, witness)
}
""",
        "the production predecessor-witness wrapper must fail closed and forward the exact barrier and witness",
        errors,
    )
    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::certified_serve_runtime_predecessor_capacity_available"
        ),
        """
pub(crate) fn certified_serve_runtime_predecessor_capacity_available(
    &self,
    barrier: CertifiedServeBarrier,
) -> Result<bool, String> {
    self.io
        .as_ref()
        .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?
        .serve_runtime_predecessor_capacity_available(barrier)
}
""",
        "the production exact-Serve capacity wrapper must fail closed and forward the exact barrier",
        errors,
    )
    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::finish_certified_serve_runtime_episode_turn"
        ),
        """
pub(crate) fn finish_certified_serve_runtime_episode_turn(
    &self,
    barrier: CertifiedServeBarrier,
    older_predecessor_remains: bool,
) -> Result<(), String> {
    self.io
        .as_ref()
        .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?
        .finish_serve_runtime_episode_turn(barrier, older_predecessor_remains)
}
""",
        "the production exact-Serve settlement wrapper must fail closed and forward the barrier and recheck result",
        errors,
    )
    _require_exact_rust_tokens(
        worker_path,
        worker_items.get(
            "ProductionV2Services::try_begin_certified_serve_producer_episode"
        ),
        """
pub(crate) fn try_begin_certified_serve_producer_episode(
    &self,
) -> Result<Option<CertifiedServeProducerEpisode>, String> {
    self.io
        .as_ref()
        .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?
        .try_begin_producer_episode()
}
""",
        "the production producer-episode wrapper must fail closed and delegate to the queue-atomic gate",
        errors,
    )

    completed = worker_items.get(
        "ProductionV2Services::take_exact_serve_predecessor_completion"
    )
    _require_rust_token_sequence(
        worker_path,
        completed,
        """
self.take_lifecycle_prefix_completion(
    runtime_capacity_available,
    serve_lifecycle_ordinal,
    false,
)
""",
        "the exact-Serve selector must route to the shared helper with a strict lifecycle cut",
        errors,
    )

    lifecycle_prefix = worker_items.get(
        "ProductionV2Services::take_lifecycle_prefix_completion"
    )
    _require_rust_token_sequence(
        worker_path,
        lifecycle_prefix,
        """
let within_cut = |ordinal: u128| {
    if inclusive {
        ordinal <= lifecycle_cut
    } else {
        ordinal < lifecycle_cut
    }
};
""",
        "the shared completion selector must distinguish inclusive timeout "
        "ownership from strict exact-Serve predecessors",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        lifecycle_prefix,
        """
owned.runtime_lifecycle_ordinal.is_some_and(|ordinal| {
    within_cut(ordinal)
        && (runtime_capacity_available || !owned.requires_runtime_capacity)
})
""",
        "I/O completion selection must apply the reviewed lifecycle cut and runtime-capacity gate",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        lifecycle_prefix,
        """
.filter(|completion| within_cut(completion.runtime_lifecycle_ordinal()))
.min_by_key(|completion| completion.runtime_lifecycle_ordinal())
""",
        "local completion selection must apply the same lifecycle cut and choose the least immutable ordinal",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        lifecycle_prefix,
        """
(Some(io), Some(local)) if io < local => Some(CompletionSource::Io),
(Some(io), Some(local)) if local < io => Some(CompletionSource::Local),
(Some(_), Some(_)) => Some(self.next_completion_source),
""",
        "completion selection must choose the least owner and retain finite fair tie-breaking",
        errors,
    )

    exact_drain = worker_items.get(
        "ProductionV2Services::drain_exact_serve_runtime_predecessor"
    )
    _require_rust_token_sequence(
        worker_path,
        exact_drain,
        """
self.drain_completions_inner(
    executor,
    1,
    CompletionDrainPolicy::ExactServePredecessor {
        serve_lifecycle_ordinal,
    },
)
""",
        "each exact-Serve predecessor turn may admit at most one completed owner",
        errors,
    )
    completion_inner = worker_items.get(
        "ProductionV2Services::drain_completions_inner"
    )
    _require_rust_token_sequence(
        worker_path,
        completion_inner,
        "while attempts < limit {",
        "every completion policy must retain its caller-supplied finite bound",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        completion_inner,
        """
CompletionDrainPolicy::ExactServePredecessor {
    serve_lifecycle_ordinal,
} => self.take_exact_serve_predecessor_completion(
    runtime_capacity_available,
    serve_lifecycle_ordinal,
),
""",
        "the exact policy must use only the strict ticket-indexed selector",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        completion_inner,
        """
CompletionDrainPolicy::TimeoutRecoveryPrefix {
    inclusive_lifecycle_cut,
} => self.take_timeout_recovery_prefix_completion(
    runtime_capacity_available,
    inclusive_lifecycle_cut,
),
""",
        "the finite completion drain must retain the separately inclusive timeout-recovery selector",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        completion_inner,
        """
let source_height = completion.artifact().height;
let source_block_hash = completion.artifact().block_hash;
let disposition = executor.complete_application(*completion, self)?;
if disposition == CompletionDisposition::Accepted {
    self.kura_replica_advert_refresh
        .note_durable_tip(
            Some((source_height, source_block_hash)),
            Instant::now(),
        )
        .map_err(|reason| executor.external_service_failed(reason, self))?;
}
""",
        "only an accepted application completion may refresh the exact durable Kura tip and refresh failures must fail closed",
        errors,
    )

    runner_item = _require_rust_item(
        runner_path,
        runner_source,
        "advance_executor_once_before_exact_serve",
        errors,
    )
    _require_rust_item_context(
        runner_path,
        runner_item,
        (),
        "one-step exact-Serve serialized transition",
        errors,
    )
    _require_rust_item_token_sha256(
        runner_path,
        runner_item,
        _EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256[
            "advance_executor_once_before_exact_serve"
        ],
        "one-step exact-Serve serialized transition",
        errors,
    )
    _require_rust_token_sequence(
        runner_path,
        runner_item,
        """
executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())?;
let _ = executor.step(Instant::now(), services)?;
""",
        "one exact-Serve turn must execute at most one serialized transition",
        errors,
    )

    effect_items: dict[str, RustItem | None] = {}
    effect_contexts = {
        "publish_external_lifecycle_owners": (
            ("impl", "<", "R", ":", "EffectRuntime", ">", "V2EffectExecutor", "<", "R", ">"),
        ),
        "older_runtime_lifecycle_predates_retained_response": (
            ("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),
        ),
        "exact_serve_predecessor_episode_witness": (
            ("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),
        ),
    }
    obsolete_effect_boolean_items = tuple(
        item
        for item in rust_items(
            effects_source,
            "older_runtime_lifecycle_predates_exact_serve",
        )
        if item.brace_context
        == (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),)
    )
    if obsolete_effect_boolean_items:
        errors.append(
            f"{effects_path}: exact-Serve execution must expose only the "
            "witness publisher; the duplicate executor boolean projection "
            "must remain absent"
        )
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256.items()
    ):
        item = _require_rust_item(effects_path, effects_source, name, errors)
        effect_items[name] = item
        _require_rust_item_context(
            effects_path,
            item,
            effect_contexts[name],
            f"exact-Serve executor seam {name}",
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            item,
            expected_sha256,
            f"exact-Serve executor seam {name}",
            errors,
        )
    _require_exact_rust_tokens(
        effects_path,
        effect_items.get("publish_external_lifecycle_owners"),
        """
fn publish_external_lifecycle_owners(&mut self) -> Result<(), EffectExecutorError> {
    let owners = self.external_lifecycle_owners()?;
    self.runtime
        .set_external_lifecycle_owners(owners)
        .map_err(EffectExecutorError::Runtime)
}
""",
        "exact-Serve owner publication must snapshot every executor-retained owner and preserve the runtime error boundary",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        effect_items.get("older_runtime_lifecycle_predates_retained_response"),
        """
pub(crate) fn older_runtime_lifecycle_predates_retained_response(
    &mut self,
    now: Instant,
    target_lifecycle_ordinal: u128,
) -> Result<bool, EffectExecutorError> {
    self.ensure_open()?;
    self.publish_external_lifecycle_owners()?;
    self.runtime
        .older_lifecycle_predates_retained_response(now, target_lifecycle_ordinal)
        .map_err(EffectExecutorError::Runtime)
}
""",
        "the retained-response probe must publish complete external ownership and delegate only to its isolated runtime state",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        effect_items.get("exact_serve_predecessor_episode_witness"),
        """
pub(crate) fn exact_serve_predecessor_episode_witness(
    &mut self,
    now: Instant,
    serve_lifecycle_ordinal: u128,
    completion_evidence: Option<ExactServePredecessorCompletionEvidence>,
) -> Result<Option<ExactServePredecessorEpisodeWitness>, EffectExecutorError> {
    self.ensure_open()?;
    self.publish_external_lifecycle_owners()?;
    self.runtime
        .exact_serve_predecessor_episode_witness(
            now,
            serve_lifecycle_ordinal,
            completion_evidence,
        )
        .map_err(EffectExecutorError::Runtime)
}
""",
        "the primary executor witness publisher must fail closed, publish every "
        "external owner first, forward exact completion evidence, and retain the "
        "runtime error boundary",
        errors,
    )

    runtime_context = (
        ("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),
    )
    witness_items: dict[str, RustItem | None] = {}
    for qualified_name, expected_sha256 in (
        _EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256.items()
    ):
        owner, name = qualified_name.rsplit("::", 1)
        witness_context = (("impl", owner),)
        matching = [
            item
            for item in rust_items(runtime_source, name)
            if item.brace_context == witness_context
        ]
        item = matching[0] if len(matching) == 1 else None
        if item is None:
            errors.append(
                f"{runtime_path}: require exactly one process-local witness method "
                f"named {qualified_name}; found {len(matching)}"
            )
        witness_items[qualified_name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            witness_context,
            f"exact-Serve predecessor witness seam {qualified_name}",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"exact-Serve predecessor witness seam {qualified_name}",
            errors,
        )
    _require_rust_token_sequence(
        runtime_path,
        witness_items.get("ExactServePredecessorEpisodeWitness::try_new"),
        """
let witness = Self {
    serve_lifecycle_ordinal,
    predecessor_lifecycle_ordinal,
    episode,
};
witness.validate_exact().then_some(witness)
""",
        "witness construction must validate the complete immutable evidence before publication",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        witness_items.get("ExactServePredecessorEpisodeWitness::validate_exact"),
        """
self.serve_lifecycle_ordinal > 0
    && self.predecessor_lifecycle_ordinal > 0
    && self.predecessor_lifecycle_ordinal < self.serve_lifecycle_ordinal
    && self.episode > 0
""",
        "witness validation must require nonzero target, strict nonzero predecessor, and nonzero episode",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        witness_items.get("ExactServePredecessorCompletionEvidence::try_new"),
        """
let evidence = Self {
    lifecycle_ordinal,
    lifecycle_ordinal_complement: !lifecycle_ordinal,
};
evidence.validate_exact().then_some(evidence)
""",
        "completion-evidence construction must derive its exact integrity "
        "complement and validate the whole process-local carrier",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        witness_items.get("ExactServePredecessorCompletionEvidence::validate_exact"),
        """
self.lifecycle_ordinal > 0
    && self.lifecycle_ordinal_complement == !self.lifecycle_ordinal
""",
        "completion evidence must reject zero or a mismatched integrity complement",
        errors,
    )
    _require_exact_rust_tokens(
        runtime_path,
        witness_items.get("ExactServePredecessorCompletionEvidence::lifecycle_ordinal"),
        """
pub(crate) const fn lifecycle_ordinal(self) -> u128 {
    self.lifecycle_ordinal
}
""",
        "completion evidence must project exactly its validated lifecycle ordinal",
        errors,
    )

    runtime_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256.items()
    ):
        item = _require_rust_item(runtime_path, runtime_source, name, errors)
        runtime_items[name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            runtime_context,
            f"exact-Serve runtime seam {name}",
            errors,
            expected_attributes=("#[cfg(test)]",)
            if name == "older_lifecycle_predates_exact_serve"
            else (),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"exact-Serve runtime seam {name}",
            errors,
        )
    _require_rust_token_sequence(
        runtime_path,
        runtime_items.get("with_driver_and_lifecycle_ordinals"),
        """
exact_serve_target_ordinal: None,
exact_serve_predecessor_retry_attempted: false,
retained_response_predecessor_target_ordinal: None,
retained_response_predecessor_retry_attempted: false,
exact_serve_predecessor_physically_present: false,
exact_serve_predecessor_episode: 0,
exact_serve_predecessor_witness: None,
""",
        "runtime construction must initialize the isolated retained-response "
        "probe and selected-Serve predecessor episode without synthetic state",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_items.get("step"),
        """
if self
    .exact_serve_target_ordinal
    .is_some_and(|target| owner.lifecycle_ordinal() < target)
{
    self.exact_serve_predecessor_retry_attempted = true;
}
if self
    .retained_response_predecessor_target_ordinal
    .is_some_and(|target| owner.lifecycle_ordinal() < target)
{
    self.retained_response_predecessor_retry_attempted = true;
}
""",
        "one retry-unadmitted FIFO attempt must latch each independently active exact target whose ordinal it predates",
        errors,
    )
    runtime_minimum = runtime_items.get("minimum_active_lifecycle_ordinal")
    _require_rust_token_sequence(
        runtime_path,
        runtime_minimum,
        "self.minimum_active_lifecycle_ordinal_excluding(&[])",
        "the exact-Serve runtime minimum must exclude no active owner",
        errors,
    )
    runtime_minimum_excluding = runtime_items.get(
        "minimum_active_lifecycle_ordinal_excluding"
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_minimum_excluding,
        "let _ = self.ingress.oldest_active_lifecycle_ordinal()?;",
        "the exact-Serve runtime minimum must deeply validate every FIFO and "
        "latent Local FIFO owner",
        errors,
    )
    runtime_runnable = runtime_items.get("minimum_runnable_lifecycle_ordinal")
    _require_rust_token_sequence(
        runtime_path,
        runtime_runnable,
        """
fn minimum_runnable_lifecycle_ordinal(
    &self,
    now: Instant,
    completion_evidence: Option<ExactServePredecessorCompletionEvidence>,
) -> Result<Option<u128>, EnqueueError>
""",
        "the runnable minimum must accept only the internal exact completion-evidence carrier",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_runnable,
        """
let _ = self.minimum_active_lifecycle_ordinal()?;
let mut minimum = self.ingress.oldest_lifecycle_ordinal()?;
""",
        "exact-Serve predecessor selection must deeply validate all owners before projecting runnable FIFO work",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_runnable,
        """
if self.driver.deferred_work_is_serviceable() {
    for admission_ordinal in self.eligible_deferred_admission_ordinals()? {
""",
        "serviceable deferred work must participate in the runnable exact-Serve predecessor minimum",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_runnable,
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
        "completion evidence must validate its integrity and shared-source mint "
        "before joining the exact least runnable-owner minimum",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        runtime_minimum_excluding,
        """
let mut minimum = self
    .ingress
    .dormant_local_fifo_reservations
    .iter()
    .map(|reservation| reservation.admission_ordinal)
    .min();
""",
        "the exact-Serve runtime minimum must retain latent Local FIFO "
        "reservations as unconditional predecessors",
        errors,
    )
    runtime_uses = runtime_items.get("active_lifecycle_uses_ordinal")
    _require_rust_token_sequence(
        runtime_path,
        runtime_uses,
        """
if self.ingress.uses_lifecycle_ordinal(lifecycle_ordinal)? {
    return Ok(true);
}
""",
        "exact-Serve collision checks must include bounded-ingress and dormant owners",
        errors,
    )

    ingress_context = (
        (
            "impl",
            "<",
            "C",
            ":",
            "ExactRuntimeCommandIdentity",
            ">",
            "BoundedIngress",
            "<",
            "C",
            ">",
        ),
    )
    ingress_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_INGRESS_ITEM_SHA256.items()
    ):
        matching = [
            item
            for item in rust_items(runtime_source, name)
            if item.brace_context == ingress_context
        ]
        item = matching[0] if len(matching) == 1 else None
        if item is None:
            errors.append(
                f"{runtime_path}: require exactly one real bounded-ingress "
                f"exact-Serve ownership method named {name}; found {len(matching)}"
            )
        ingress_items[name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            ingress_context,
            f"exact-Serve bounded-ingress ownership seam {name}",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"exact-Serve bounded-ingress ownership seam {name}",
            errors,
        )
    _require_rust_token_sequence(
        runtime_path,
        ingress_items.get("oldest_active_lifecycle_ordinal"),
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
        "latent Local FIFO reservations must retain exact minted identity but "
        "remain passive until a runnable occurrence materializes",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        ingress_items.get("uses_lifecycle_ordinal"),
        """
if self
    .dormant_local_fifo_reservations
    .iter()
    .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)
{
    return Ok(true);
}
""",
        "latent Local FIFO reservations must collide with reused exact-Serve ordinals",
        errors,
    )
    runtime_witness = runtime_items.get(
        "exact_serve_predecessor_episode_witness"
    )
    for sequence, description in (
        (
            """
pub(crate) fn exact_serve_predecessor_episode_witness(
    &mut self,
    now: Instant,
    serve_lifecycle_ordinal: u128,
    completion_evidence: Option<ExactServePredecessorCompletionEvidence>,
) -> Result<Option<ExactServePredecessorEpisodeWitness>, String>
""",
            "the witness producer must accept only one internal completion-evidence carrier",
        ),
        (
            "self.ingress.lifecycle_ordinals.recognizes_minted(serve_lifecycle_ordinal)",
            "the exact ticket ordinal must come from the shared actor-global source",
        ),
        (
            "self.freeze_due_clock_owners(now)",
            "due clocks must acquire immutable ownership before comparison",
        ),
        (
            "self.active_lifecycle_uses_ordinal(serve_lifecycle_ordinal)",
            "ticket/runtime ordinal collisions must fail closed",
        ),
        (
            """
if completion_evidence.is_some_and(|evidence| {
    !evidence.validate_exact()
        || evidence.lifecycle_ordinal() >= serve_lifecycle_ordinal
}) {
    self.latch_fail_closed(
        "exact Serve completion evidence was invalid or did not strictly precede its target",
    );
    return Err("Sumeragi v2 exact Serve completion evidence was invalid".to_owned());
}
""",
            "completion evidence must be exact and strictly older than its immutable Serve target",
        ),
        (
            """
if self.exact_serve_target_ordinal != Some(serve_lifecycle_ordinal) {
    self.exact_serve_target_ordinal = Some(serve_lifecycle_ordinal);
    self.exact_serve_predecessor_retry_attempted = false;
    self.exact_serve_predecessor_physically_present = false;
    self.exact_serve_predecessor_episode = 0;
    self.exact_serve_predecessor_witness = None;
}
""",
            "a different exact target must reset every process-local predecessor-episode component",
        ),
        (
            """
let minimum = match self.minimum_runnable_lifecycle_ordinal(now, completion_evidence) {
""",
            "exact-Serve comparison must use only owners runnable by one serialized turn",
        ),
        (
            "let predecessor = minimum.filter(|ordinal| *ordinal < serve_lifecycle_ordinal);",
            "the witness producer must retain only a strictly older runnable minimum",
        ),
        (
            """
if self.exact_serve_predecessor_retry_attempted {
    if predecessor.is_some() {
        self.exact_serve_predecessor_physically_present = true;
    } else {
        self.exact_serve_predecessor_retry_attempted = false;
        self.exact_serve_predecessor_physically_present = false;
        self.exact_serve_predecessor_witness = None;
    }
    return Ok(None);
}
""",
            "retry-unadmitted suppression must retain physical presence and cannot mint another witness",
        ),
        (
            """
let Some(predecessor_lifecycle_ordinal) = predecessor else {
    self.exact_serve_predecessor_physically_present = false;
    self.exact_serve_predecessor_witness = None;
    return Ok(None);
};
""",
            "observing no older runnable owner must close the continuous physical episode before returning none",
        ),
        (
            """
if !self.exact_serve_predecessor_physically_present {
    let Some(episode) = self.exact_serve_predecessor_episode.checked_add(1) else {
""",
            "only an observed absence-to-presence transition may checked-increment the producer episode",
        ),
        (
            """
let Some(witness) = ExactServePredecessorEpisodeWitness::try_new(
    serve_lifecycle_ordinal,
    predecessor_lifecycle_ordinal,
    episode,
) else {
""",
            "a new producer episode must bind the exact target and predecessor through validated witness construction",
        ),
        (
            """
self.exact_serve_predecessor_episode = episode;
self.exact_serve_predecessor_witness = Some(witness);
self.exact_serve_predecessor_physically_present = true;
""",
            "the witness, episode, and physical-presence latch must publish atomically in one serialized actor turn",
        ),
        (
            """
if !witness.validate_exact()
    || witness.serve_lifecycle_ordinal() != serve_lifecycle_ordinal
{
""",
            "every stable witness return must revalidate the exact Serve target identity",
        ),
        (
            "Ok(Some(witness))",
            "a continuous predecessor prefix must return its stable retained witness",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            runtime_witness,
            sequence,
            description,
            errors,
        )

    _require_exact_rust_tokens(
        runtime_path,
        runtime_items.get("older_lifecycle_predates_exact_serve"),
        """
pub(crate) fn older_lifecycle_predates_exact_serve(
    &mut self,
    now: Instant,
    serve_lifecycle_ordinal: u128,
) -> Result<bool, String> {
    self.exact_serve_predecessor_episode_witness(now, serve_lifecycle_ordinal, None)
        .map(|witness| witness.is_some())
}
""",
        "the selected-Serve boolean projection must delegate exclusively to the witnessed producer seam",
        errors,
    )

    retained_response_probe = runtime_items.get(
        "older_lifecycle_predates_retained_response"
    )
    for sequence, description in (
        (
            "self.ingress.lifecycle_ordinals.recognizes_minted(serve_lifecycle_ordinal)",
            "the retained-response target must come from the shared actor-global source",
        ),
        (
            "self.freeze_due_clock_owners(now)",
            "the retained-response probe must freeze due clocks before comparison",
        ),
        (
            "self.active_lifecycle_uses_ordinal(serve_lifecycle_ordinal)",
            "the retained-response target must reject runtime ordinal collisions",
        ),
        (
            """
if self.retained_response_predecessor_target_ordinal != Some(serve_lifecycle_ordinal) {
    self.retained_response_predecessor_target_ordinal = Some(serve_lifecycle_ordinal);
    self.retained_response_predecessor_retry_attempted = false;
}
""",
            "the retained-response probe must reset only its own target and retry latch",
        ),
        (
            "self.minimum_runnable_lifecycle_ordinal(now, None)",
            "the retained-response probe must compare the complete runnable owner minimum",
        ),
        (
            """
let predecessor_exists =
    minimum.is_some_and(|ordinal| ordinal < serve_lifecycle_ordinal);
if self.retained_response_predecessor_retry_attempted {
    if !predecessor_exists {
        self.retained_response_predecessor_retry_attempted = false;
    }
    return Ok(false);
}
Ok(predecessor_exists)
""",
            "the retained-response target must grant at most one attempt for its strict older prefix",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            retained_response_probe,
            sequence,
            description,
            errors,
        )

    for item, forbidden_fields, description in (
        (
            runtime_witness,
            (
                "retained_response_predecessor_target_ordinal",
                "retained_response_predecessor_retry_attempted",
            ),
            "selected-Serve witness producer",
        ),
        (
            retained_response_probe,
            (
                "exact_serve_target_ordinal",
                "exact_serve_predecessor_retry_attempted",
                "exact_serve_predecessor_physically_present",
                "exact_serve_predecessor_episode",
                "exact_serve_predecessor_witness",
            ),
            "retained-response predecessor probe",
        ),
    ):
        if item is None:
            continue
        tokens = rust_code_tokens(item.source)
        present = [
            field
            for field in forbidden_fields
            if _token_sequence_positions(tokens, rust_code_tokens(field))
        ]
        if present:
            errors.append(
                f"{runtime_path}:{item.line}: {description} must not mutate or "
                f"read the other target's episode state; found {present!r}"
            )

    run_inner = _require_rust_item(
        runner_path,
        runner_source,
        "run_inner",
        errors,
    )
    _require_rust_item_context(
        runner_path,
        run_inner,
        (),
        "selected exact-Serve witness observation ordering",
        errors,
        expected_attributes=("#[allow(clippy::too_many_lines)]",),
    )
    _require_rust_item_token_sha256(
        runner_path,
        run_inner,
        _EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256["run_inner"],
        "selected exact-Serve witness observation ordering",
        errors,
    )
    for sequence, description in (
        (
            """
let completion_evidence = services
    .certified_serve_predecessor_completion_evidence(
        executor.remaining_completion_capacity() != 0,
        serve_barrier.scheduler_ordinal(),
    )
    .map_err(V2RunnerError::Service)?;
if let Some(witness) = executor.exact_serve_predecessor_episode_witness(
    Instant::now(),
    serve_barrier.scheduler_ordinal(),
    completion_evidence,
)? {
    let _ = services
        .observe_certified_serve_predecessor_episode_witness(serve_barrier, witness)
        .map_err(V2RunnerError::Service)?;
}
let claimed_older_runtime_episode = services
    .claim_certified_serve_runtime_episode(serve_barrier)
    .map_err(V2RunnerError::Service)?;
if claimed_older_runtime_episode {
    services.drain_exact_serve_runtime_predecessor(
        &mut executor,
        serve_barrier.scheduler_ordinal(),
    )?;
""",
            "the runner must publish and consume a late predecessor witness before attempting to claim a sealed exact target",
        ),
        (
            """
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
if predecessor_witness.is_some()
    && services
        .certified_serve_runtime_predecessor_capacity_available(serve_barrier)
        .map_err(V2RunnerError::Service)?
{
""",
            "the serialized predecessor step must consume the stable witness and require both that witness and physical capacity",
        ),
        (
            """
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
""",
            "every claimed turn must re-publish, consume, and recheck the full witnessed owner set before settlement",
        ),
        (
            """
let Some(_certified_serve_producer_episode) = services
    .try_begin_certified_serve_producer_episode()
    .map_err(V2RunnerError::Service)?
else {
    let _ = wake_rx.recv_timeout(IDLE_POLL);
    continue;
};
""",
            "the queue-locked handoff to an exact target which won the admission race must retain the finite wake bound",
        ),
    ):
        _require_rust_token_sequence(
            runner_path,
            run_inner,
            sequence,
            description,
            errors,
        )

    _require_rust_token_sequence(
        runner_path,
        run_inner,
        """
services.drain_exact_serve_runtime_predecessor(
    &mut executor,
    serve_barrier.scheduler_ordinal(),
)?
""",
        "runner must drain exactly one strict completion only inside the "
        "successfully claimed selected-Serve predecessor episode",
        errors,
        count=1,
    )

    _require_rust_token_sequence(
        runner_path,
        run_inner,
        """
services.certified_serve_predecessor_completion_evidence(
    executor.remaining_completion_capacity() != 0,
    serve_barrier.scheduler_ordinal(),
)
""",
        "the runner must freshly project exact completion ownership before "
        "each of its three selected-Serve witness observations",
        errors,
        count=3,
    )

    expected_test_context = (
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
    worker_regression_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(worker_path, worker_source, name, errors)
        worker_regression_items[name] = item
        _require_rust_item_context(
            worker_path,
            item,
            expected_test_context,
            f"exact-Serve runtime-episode regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            worker_path,
            item,
            expected_sha256,
            f"exact-Serve runtime-episode regression {name}",
            errors,
        )

    for name, sequence, description, count in (
        (
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            "for fresh_ticket_ordinal in first_ticket_ordinal..=later_ordinal",
            "the strict exact-Serve regression must exclude an equal-or-later completion through every non-newer ticket",
            1,
        ),
        (
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            """
service
    .certified_serve_predecessor_completion_evidence(
        true,
        first_ticket_ordinal,
    )
    .expect("project the completed predecessor without consuming it")
    .map(ExactServePredecessorCompletionEvidence::lifecycle_ordinal),
Some(older_task.lifecycle_ordinal()),
""",
            "the completion-evidence regression must project the least strict local predecessor",
            1,
        ),
        (
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            """
service
    .certified_serve_predecessor_completion_evidence(
        false,
        first_ticket_ordinal,
    )
    .expect("project the capacity-blocked predecessor")
    .is_none()
""",
            "a completion requiring runtime capacity must not reopen Serve while capacity is unavailable",
            1,
        ),
        (
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            "service.take_exact_serve_predecessor_completion(true, later_ordinal + 1)",
            "the strict exact-Serve regression must release the later completion only after a strictly newer ticket",
            1,
        ),
        (
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            """
service
    .certified_serve_predecessor_completion_evidence(true, later_ordinal + 1)
    .expect("project the newly strict I/O predecessor")
    .map(ExactServePredecessorCompletionEvidence::lifecycle_ordinal),
Some(later_ordinal),
""",
            "the I/O completion must become evidence only below a strictly later ticket",
            1,
        ),
        (
            "repeated_exact_serve_claims_close_all_older_sources_before_later_io",
            """
service
    .finish_certified_serve_runtime_episode_turn(barrier, false)
    .expect("later-rank I/O cannot keep the older-owner episode open");
assert!(
    !service
        .claim_certified_serve_runtime_episode(barrier)
        .expect("completed episode cannot be reclaimed")
);
            """,
            "repeated exact-Serve claims must stay sealed after the complete older-owner set is exhausted unless a newer witness arrives",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
let first = ExactServePredecessorEpisodeWitness::for_test(
    barrier.scheduler_ordinal(),
    1,
    1
);
""",
            "the witness regression must begin with exact predecessor one at episode one",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
let conflicting = ExactServePredecessorEpisodeWitness::for_test(
    barrier.scheduler_ordinal(),
    2,
    1
);
""",
            "the witness regression must model a same-episode exact-evidence conflict",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
let skipped = ExactServePredecessorEpisodeWitness::for_test(
    barrier.scheduler_ordinal(),
    2,
    3
);
""",
            "the witness regression must model a skipped producer episode",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
let replenished = ExactServePredecessorEpisodeWitness::for_test(
    barrier.scheduler_ordinal(),
    2,
    2
);
""",
            "the witness regression must model the exact next producer episode",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
command_tx
    .finish_serve_runtime_episode_turn(barrier, false)
    .expect("seal exhausted initial predecessor turn");
assert!(
    !command_tx
        .observe_serve_predecessor_episode_witness(barrier, first)
        .expect("same physical episode must coalesce")
);
assert!(
    !command_tx
        .claim_serve_runtime_episode(barrier)
        .expect("same witness cannot reopen a completed turn")
);
""",
            "the witness regression must prove that Complete remains sealed for an identical episode",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
command_tx
    .observe_serve_predecessor_episode_witness(barrier, conflicting)
    .is_err()
""",
            "the witness regression must reject conflicting evidence within one episode",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
command_tx
    .observe_serve_predecessor_episode_witness(barrier, skipped)
    .is_err()
""",
            "the witness regression must reject a skipped consumer episode",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
assert!(
    command_tx
        .observe_serve_predecessor_episode_witness(barrier, replenished)
        .expect("strictly newer runtime witness reopens the target")
);
assert!(
    command_tx
        .claim_serve_runtime_episode(barrier)
        .expect("claim exactly one replenished predecessor turn")
);
assert!(
    !command_tx
        .observe_serve_predecessor_episode_witness(barrier, replenished)
        .expect("repeated replenishment witness must stutter")
);
""",
            "exactly the next witness must reopen Complete once and then stutter",
            1,
        ),
        (
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            """
let (admission, committed) = drain_and_commit_gated_serve(
    &ingress,
    &command_tx,
    CertifiedServeOwnerKey::Roster(requester),
    &request,
);
assert!(matches!(committed, CertifiedServeCommit::Queued));
assert!(matches!(
    command_rx.try_recv(),
    Ok(V2IoCommand::Serve { lifecycle_id, .. })
        if lifecycle_id == admission.lifecycle_id
));
let producer_episode = command_tx
    .try_begin_producer_episode()
    .expect("consume the post-Serve producer handoff")
    .expect("final target retirement owes one producer episode");
drop(producer_episode);
""",
            "the reopened owner must retire through real target delivery and the finite producer handoff",
            1,
        ),
        (
            "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission",
            """
assert!(
    !command_tx
        .serve_runtime_predecessor_capacity_available(barrier)
        .expect("inspect the full frozen prefix"),
    "the runner must wait instead of dispatching a retained effect into a full queue"
);
""",
            "a full Control prefix must deny predecessor capacity until its sole physical slot drains",
            1,
        ),
        (
            "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission",
            """
command_tx.try_send(sign(2)),
Err(V2IoTrySendError::Full(_))
""",
            "an equal-ticket causal owner must remain outside both the frozen and transferred exact-Serve prefix",
            2,
        ),
        (
            "worker_completion_is_retained_behind_a_full_runtime_fifo",
            """
for expected_debt in 1..=3 {
    assert!(
        io.record_completion_service_attempt(0),
        "the full runtime FIFO must retain the oldest worker completion"
    );
""",
            "a full runtime FIFO must retain the oldest worker completion and accrue bounded service debt",
            1,
        ),
        (
            "worker_completion_is_retained_behind_a_full_runtime_fifo",
            """
let snapshot = io.completion_snapshot(snapshot_at);
assert_eq!(snapshot.depth, 1);
""",
            "a full runtime FIFO must retain the exact oldest worker completion while service debt grows",
            1,
        ),
        (
            "production_drain_publishes_worker_completion_behind_full_runtime_fifo",
            """
service.retire_held_io_completion();
let drained = service
    .io
    .as_ref()
    .expect("attached completion owner")
    .completion_snapshot(Instant::now());
assert_eq!(drained.depth, 1);
assert!(
    !command_rx.queue.lock().work.contains_key(&work_id),
    "retiring the consumed held result acknowledges exact work ownership"
);
""",
            "production completion drain must explicitly acknowledge the exact held worker result",
            1,
        ),
        (
            "production_drain_publishes_worker_completion_behind_full_runtime_fifo",
            """
let second = service
    .last_status
    .as_ref()
    .expect("repeated backpressure republishes effect status");
assert_eq!(second.effect_completion_queue.depth, 2);
assert_eq!(second.effect_completion_queue.max_service_debt, 2);
""",
            "production completion drain must retain and republish the full worker prefix under repeated runtime backpressure",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            """
gate.reserve(request.request(), &via, true, 2),
Err(CertifiedServeIngressReserveError::Busy)
""",
            "a drained exact retransmission must wait for the owed producer episode before reserving a new carrier",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            """
command_tx
    .queue
    .lifecycle_ordinals
    .next_ordinal_for_test(),
scheduler_before_retry,
""",
            "a blocked exact retransmission must not mint an actor-global ordinal",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            """
let post_drain_producer_episode = command_tx
    .try_begin_producer_episode()
    .expect("consume the post-drain producer handoff")
    .expect("final Serve retirement owes one producer episode");
drop(post_drain_producer_episode);
""",
            "a drained exact retransmission must consume the atomic producer handoff before readmission",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            "retry_barrier.scheduler_ordinal() > first_barrier.scheduler_ordinal()",
            "a drained exact retransmission must receive a fresh scheduler ordinal",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            """
retry_barrier.lifecycle_id(),
first_barrier.lifecycle_id(),
""",
            "a drained exact retransmission must retain its immutable logical lifecycle",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            "retry_barrier.carrier_ordinal() > first_barrier.carrier_ordinal()",
            "a drained exact retransmission must receive a fresh physical carrier ordinal",
            1,
        ),
        (
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            "assert!(matches!(retry_commit, CertifiedServeCommit::Coalesced));",
            "a drained exact retransmission must resolve through its retained logical lifecycle",
            1,
        ),
        (
            "certified_serve_future_slot_blocks_control_and_consensus_replenishment",
            """
for class in [V2IoAdmissionClass::Consensus, V2IoAdmissionClass::Control] {
""",
            "a reserved future Serve slot must block both later Consensus and Control replenishment",
            1,
        ),
    ):
        _require_rust_token_sequence(
            worker_path,
            worker_regression_items.get(name),
            sequence,
            description,
            errors,
            count=count,
        )

    producer_handoff_regression = _require_rust_item(
        worker_path,
        worker_source,
        "final_serve_retirement_yields_one_producer_episode_before_replenishment",
        errors,
    )
    for sequence, description, count in (
        (
            """
assert!(state.producer_episode_due);
assert!(!state.producer_episode_active);
assert!(state.serve_ingress_reservation.is_none());
assert!(state.serve_ingress_waiters.is_empty());
""",
            "the handoff regression must observe due only after the complete frozen Serve batch retires",
            1,
        ),
        (
            """
gate.reserve(replenishment.request(), &via, true, 3),
Err(CertifiedServeIngressReserveError::Busy)
""",
            "the handoff regression must reject replenishment both before and during the producer episode",
            2,
        ),
        (
            """
command_tx.queue.lifecycle_ordinals.next_ordinal_for_test(),
actor_ordinal_before,
""",
            "the handoff regression must preserve the actor-global ordinal while replenishment is blocked",
            1,
        ),
        (
            """
command_tx.queue.lock().next_serve_admission_ordinal,
lifecycle_ordinal_before,
""",
            "the handoff regression must preserve the logical lifecycle high-watermark while replenishment is blocked",
            1,
        ),
        (
            """
let producer_episode = command_tx
    .try_begin_producer_episode()
    .expect("consume the atomic post-Serve handoff")
    .expect("the final frozen Serve batch owes one producer episode");
""",
            "the handoff regression must consume the owed producer episode through the production queue gate",
            1,
        ),
        (
            """
drop(producer_episode);
{
    let state = command_tx.queue.lock();
    assert!(!state.producer_episode_due);
    assert!(!state.producer_episode_active);
}

assert!(matches!(
    ingress.try_push(certified_serve_inbound_with_route(
        replenishment.request(),
        via,
        replenishment_route,
    )),
    Ok(FairV2IngressPushDisposition::Enqueued)
));
""",
            "the handoff regression must reopen Serve admission only after the bounded producer lease retires",
            1,
        ),
    ):
        _require_rust_token_sequence(
            worker_path,
            producer_handoff_regression,
            sequence,
            description,
            errors,
            count=count,
        )

    runtime_test_context = (
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
    )
    effect_regression_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(effects_path, effects_source, name, errors)
        effect_regression_items[name] = item
        _require_rust_item_context(
            effects_path,
            item,
            runtime_test_context,
            f"late passive-Fetch exact-Serve regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            effects_path,
            item,
            expected_sha256,
            f"late passive-Fetch exact-Serve regression {name}",
            errors,
        )

    late_passive_fetch = effect_regression_items.get(
        "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps"
    )
    for sequence, description in (
        (
            """
let fetch_ordinal = fixture
    .lifecycle_ordinals
    .reserve_one()
    .expect("reserve the passive Fetch lifecycle before Serve");
""",
            "the concrete late-runnable regression must reserve passive Fetch ownership before the Serve target",
        ),
        (
            """
let serve_ordinal = fixture
    .lifecycle_ordinals
    .reserve_one()
    .expect("reserve the selected Serve target after Fetch");
assert!(
fixture
    .executor
    .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
    .expect("observe the selected Serve before Fetch completion")
    .is_none(),
    "passive Fetch transport work alone cannot block Serve"
);
""",
            "a passive Fetch alone must not mint or block on a predecessor witness",
        ),
        (
            """
fixture
    .executor
    .complete_body_reconstruction(&task, manifest, body, &mut services)
    .expect("late reconstruction materializes BodyAvailable under the Fetch owner");
let witness = fixture
    .executor
    .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
    .expect("observe late BodyAvailable behind the selected Serve")
    .expect("late runnable predecessor reopens the completed Serve episode");
assert_eq!(witness.serve_lifecycle_ordinal(), serve_ordinal);
assert_eq!(witness.predecessor_lifecycle_ordinal(), fetch_ordinal);
assert_eq!(witness.episode(), 1);
""",
            "late BodyAvailable materialization must mint episode one for the original earlier Fetch owner",
        ),
        (
            """
let retained_response_ordinal = fixture
    .lifecycle_ordinals
    .reserve_one()
    .expect("reserve an isolated retained-response target after Serve");
assert!(
    fixture
        .executor
        .older_runtime_lifecycle_predates_retained_response(
            Instant::now(),
            retained_response_ordinal,
        )
        .expect("exercise the published retained-response predecessor probe")
);
""",
            "the concrete late-runnable regression must execute the published isolated retained-response wrapper",
        ),
        (
            """
fixture
    .executor
    .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
    .expect("retained-response probing cannot reset the selected-Serve witness"),
Some(witness),
""",
            "one continuous late predecessor prefix must retain the identical witness across the alternate target probe",
        ),
        (
            """
assert_eq!(
    fixture.executor.status().queued_runtime_completions,
    1,
    "the late Fetch successor is runnable inside serialized runtime"
);

assert!(matches!(
    fixture
        .executor
        .step(Instant::now(), &mut services)
        .expect("the reopened predecessor owns the next serialized step"),
    EffectExecutorStep::Advanced { .. }
));
assert_eq!(fixture.executor.status().queued_runtime_completions, 0);
""",
            "the reopened predecessor must consume one real runtime completion in one serialized step",
        ),
        (
            """
assert_eq!(
    services.store_tasks.len(),
    1,
    "the reopened BodyAvailable transition must produce one Store successor"
);
""",
            "the reopened BodyAvailable transition must produce exactly one Store successor",
        ),
        (
            """
assert_eq!(
    services.store_tasks[0].lifecycle_ordinal(),
    fetch_ordinal,
    "the Store successor must keep the reopened Fetch owner"
);
""",
            "the Store successor must retain the immutable original Fetch owner",
        ),
        (
            """
assert!(fixture.executor.pending_fetches.is_empty());
assert!(
    fixture
        .executor
        .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
        .expect("an incomplete Store remains passive")
        .is_none(),
    "pending Store work alone cannot reopen the Serve episode"
);
""",
            "an incomplete asynchronous Store must remain passive and cannot veto Serve",
        ),
        (
            """
let stored_completion_evidence =
    ExactServePredecessorCompletionEvidence::try_new(fetch_ordinal)
        .expect("tracked Store completion retains the exact Fetch ordinal");
let replenished = fixture
    .executor
    .exact_serve_predecessor_episode_witness(
        Instant::now(),
        serve_ordinal,
        Some(stored_completion_evidence),
    )
    .expect("a completed Store is runnable")
    .expect("a completed Store reopens one later Serve episode");
assert_eq!(replenished.predecessor_lifecycle_ordinal(), fetch_ordinal);
assert_eq!(replenished.episode(), 2);
""",
            "a tracked completed Store must retain its immutable owner and open "
            "exactly the next finite predecessor episode",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            late_passive_fetch,
            sequence,
            description,
            errors,
        )

    runtime_regression_items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(runtime_path, runtime_source, name, errors)
        runtime_regression_items[name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            runtime_test_context,
            f"exact-Serve latent-replay regression {name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            expected_sha256,
            f"exact-Serve latent-replay regression {name}",
            errors,
        )

    alternating_targets = runtime_regression_items.get(
        "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt"
    )
    _require_rust_token_sequence(
        runtime_path,
        alternating_targets,
        """
runtime
    .older_lifecycle_predates_retained_response(start, retained_response_ordinal)
    .expect("alternate retained-response target sees the same older owner")
""",
        "the alternating-target regression must exercise the isolated retained-response probe",
        errors,
        count=3,
    )
    for sequence, description in (
        (
            """
runtime
            .exact_serve_predecessor_episode_witness(start, serve_ordinal, None)
    .expect("alternate target cannot reset selected-Serve witness state"),
Some(first_witness),
""",
            "the selected-Serve witness must remain stable after an alternate target probe",
        ),
        (
            """
assert_eq!(runtime.exact_serve_target_ordinal, Some(serve_ordinal));
assert!(runtime.exact_serve_predecessor_retry_attempted);
assert_eq!(
    runtime.retained_response_predecessor_target_ordinal,
    Some(retained_response_ordinal)
);
assert!(runtime.retained_response_predecessor_retry_attempted);
""",
            "one retry-unadmitted step must latch both independently active exact targets",
        ),
        (
            """
assert!(!runtime.exact_serve_predecessor_retry_attempted);
assert!(!runtime.exact_serve_predecessor_physically_present);
assert!(
    !runtime
        .older_lifecycle_predates_retained_response(start, retained_response_ordinal)
        .expect("settled owner clears alternate-target retry suppression")
);
assert!(!runtime.retained_response_predecessor_retry_attempted);
""",
            "settling the shared older owner must clear both retry latches without witness regression",
        ),
        (
            """
let completed_evidence =
    ExactServePredecessorCompletionEvidence::try_new(completed_ordinal)
        .expect("completed service evidence is nonzero and exact");
let completed_target = runtime
    .ingress
    .mint_non_fifo_lifecycle_ordinal()
    .expect("new Serve target follows the completed service owner");
assert!(
    runtime
        .exact_serve_predecessor_episode_witness(start, completed_target, None)
        .expect("passive ownership alone remains absent")
        .is_none()
);
let completed_witness = runtime
    .exact_serve_predecessor_episode_witness(
        start,
        completed_target,
        Some(completed_evidence),
    )
    .expect("completion-qualified owner is accepted")
    .expect("completion-qualified owner opens one predecessor episode");
assert_eq!(
    completed_witness.predecessor_lifecycle_ordinal(),
    completed_ordinal
);
assert_eq!(completed_witness.episode(), 1);
""",
            "only exact completion evidence may turn a passive service owner "
            "into one finite runnable predecessor episode",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            alternating_targets,
            sequence,
            description,
            errors,
        )

    dormant_replay = runtime_regression_items.get(
        "restart_dormant_local_fifo_reservation_survives_full_class_churn"
    )
    for sequence, description, count in (
        (
            """
runtime.minimum_active_lifecycle_ordinal(),
Ok(Some(1)),
""",
            "restart-dormant ownership must remain in the complete active lifecycle inventory before and after materialization",
            2,
        ),
        (
            """
!runtime
    .older_lifecycle_predates_exact_serve(started_at, later_serve)
    .expect("inspect passive dormant ownership at the Serve cut")
""",
            "restart-dormant ownership must remain passive until its runnable occurrence materializes",
            1,
        ),
        (
            "assert!(runtime.ingress.dormant_local_fifo_reservations.is_empty());",
            "an exact full-capacity replay must atomically consume its dormant reservation",
            1,
        ),
        (
            "assert_eq!(selected.selected, RuntimeSelectedOwnerKind::Fifo);",
            "the materialized restart-dormant owner must dispatch as the oldest runnable FIFO owner",
            1,
        ),
        (
            """
runtime.driver.delivered,
vec![(owner_tag, 9)],
""",
            "the restored restart-dormant owner must dispatch before every younger physical command",
            1,
        ),
        (
            """
Err(EnqueueError::FailClosed),
"ReuseDormant after latent-slot removal cannot recreate the drained stage"
""",
            "a drained restart-dormant lifecycle must not resurrect after its latent slot is consumed",
            1,
        ),
        (
            "assert!(runtime.fail_closed);",
            "a rejected restart-dormant resurrection must latch the runtime fail closed",
            1,
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            dormant_replay,
            sequence,
            description,
            errors,
            count=count,
        )

    return errors
