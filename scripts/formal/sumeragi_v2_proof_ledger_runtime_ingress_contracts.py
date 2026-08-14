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
    next_context = (("impl", "OuterIngressTurns"),)
    next_items = tuple(
        item
        for item in rust_items(runner_source, "next_current")
        if item.brace_context == next_context
    )
    if len(next_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one borrow-bound outer-ingress "
            f"current-turn mint; found {len(next_items)}"
        )
        next_item = None
    else:
        next_item = next_items[0]
        _require_rust_item_context(
            runner_path,
            next_item,
            next_context,
            "borrow-bound outer-ingress current turn",
            errors,
        )
    _require_exact_rust_tokens(
        runner_path,
        next_item,
        """
fn next_current(&mut self) -> Option<LifecycleCurrentRunnerTurn<'_>> {
    if self.cycles_remaining == 0 {
        return None;
    }
    Some(LifecycleCurrentRunnerTurn {
        turn: self.next_turn,
        cursor: self,
    })
}
""",
        "outer-ingress cursor must lend exactly its current turn without advancing early",
        errors,
    )
    advance_item = _require_rust_item(
        runner_path, runner_source, "advance_current", errors
    )
    _require_exact_rust_tokens(
        runner_path,
        advance_item,
        """
fn advance_current(&mut self, turn: OuterIngressTurn) {
    assert_eq!(
        self.next_turn, turn,
        "borrow-bound outer runner turn must remain current until drop"
    );
    self.next_turn = match turn {
        OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
        OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
        OuterIngressTurn::Ingress => {
            self.cycles_remaining -= 1;
            OuterIngressTurn::Completion
        }
    };
}
""",
        "dropping a borrowed turn must advance exactly one finite Completion, Runtime, or Ingress target",
        errors,
    )
    drop_context = (
        ("impl", "Drop", "for", "LifecycleCurrentRunnerTurn", "<", "'", "_", ">"),
    )
    drop_items = tuple(
        item
        for item in rust_items(runner_source, "drop")
        if item.brace_context == drop_context
    )
    drop_item = drop_items[0] if len(drop_items) == 1 else None
    if len(drop_items) != 1:
        errors.append(
            f"{runner_path}: require exactly one current-turn Drop advance; "
            f"found {len(drop_items)}"
        )
    _require_exact_rust_tokens(
        runner_path,
        drop_item,
        """
fn drop(&mut self) {
    self.cursor.advance_current(self.turn);
}
""",
        "the current runner turn must advance only through its affine Drop",
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
