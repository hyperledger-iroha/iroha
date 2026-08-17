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
        "queue-local leader-wire gate verdict",
        errors,
    )
    _require_rust_item_token_sha256(
        ingress_path,
        queue_gate,
        _LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
            "fair_v2_ingress_queue_gate_verdict"
        ],
        "queue-local leader-wire gate verdict",
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
    &leader_wire_projection,
    barrier_bypass,
);
""",
        "every candidate must use the sealed queue-local gate verdict",
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
        "the physical selector must delegate to the shared strict-before-dependency fair choice",
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
    for item, stale_sequence, description in (
        (
            projection,
            "selected_serve_barrier",
            "leader-wire projection must not depend on a retired Serve barrier",
        ),
        (
            selector,
            "serve_projection",
            "the physical selector must not recreate retired Serve arbitration",
        ),
        (
            queue_gate,
            "selected_serve",
            "the queue-local gate must remain leader-wire-only",
        ),
    ):
        _require_rust_token_sequence(
            ingress_path,
            item,
            stale_sequence,
            description,
            errors,
            count=0,
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


def _postmerge_exact_output_strengthening_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind the merge-added recovered Apply and lifecycle-owned output seams."""

    base = repo_root / "crates" / "iroha_core" / "src"
    paths = {
        "worker": base / "sumeragi" / "v2_worker.rs",
        "effects": base / "sumeragi" / "v2_effects.rs",
        "lane": base / "sumeragi" / "v2_lane_work.rs",
        "merge": base / "merge_sidecar.rs",
    }
    errors: list[str] = []
    sources: dict[str, str] = {}
    for name, path in paths.items():
        _path, sources[name] = _read_reviewed_rust_source(
            repo_root, path.relative_to(repo_root).as_posix(), errors,
            f"post-merge exact-output {name} provider")

    reviewed_attributes = {
        "ProductionV2Services::start": (
            "#[allow(clippy::too_many_arguments, dead_code)]",
        ),
        "ProductionV2Services::start_with_apply_service": ("#[allow(clippy::too_many_arguments)]",),
        "ProductionV2Services::start_inner": ("#[allow(clippy::too_many_arguments)]",),
        "ProductionV2Services::activate_effect_completion_observer": ("#[allow(dead_code)]",),
        "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner": ("#[allow(clippy::too_many_arguments)]",),
        "MergeSidecarTransport::defer_block_with_priority": ("#[allow(clippy::too_many_arguments)]",),
    }

    def method(provider: str, owner: str, name: str) -> RustItem | None:
        return _require_qualified_rust_item(
            paths[provider], sources[provider], owner, name, errors,
            f"post-merge exact-output {owner}::{name}",
            expected_attributes=reviewed_attributes.get(f"{owner}::{name}", ()))

    start = method("worker", "ProductionV2Services", "start")
    start_with = method("worker", "ProductionV2Services", "start_with_apply_service")
    start_inner = method("worker", "ProductionV2Services", "start_inner")
    observer = method("worker", "ProductionV2Services", "activate_effect_completion_observer")
    _require_rust_token_sequence(
        paths["worker"], start_with,
        """if !state.matches_kura_instance(&kura)
|| !apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)
{ return Err("Sumeragi v2 recovered Apply service changed lifecycle identity".to_owned(),); }
Self::start_inner(""",
        "recovered startup must authenticate State, Kura, context, and proof roster before sharing the constructor",
        errors)
    for item, description in (
        (start, "ordinary startup"), (start_with, "recovered startup"),
        (start_inner, "shared startup"),
    ):
        _require_rust_token_sequence(
            paths["worker"], item, "activate_effect_completion_observer",
            f"{description} must not activate the completion observer before its move-only permit", errors,
            count=0)
    _require_rust_token_sequence(
        paths["worker"], observer,
        """_permit: ProductionV2CompletionObserverActivationPermitV1,
) -> Result<(), String> { let activation_guard = Arc::clone(&self.output_guard);
let activation = activation_guard.begin_fail_stop_operation()""",
        "completion observer activation must require its opaque permit and arm fail-stop first",
        errors)

    complete_apply = _require_rust_item(
        paths["effects"], sources["effects"], "complete_application", errors)
    _require_rust_item_context(
        paths["effects"], complete_apply,
        (("impl", "<", "R", ":", "EffectRuntime", ">", "V2EffectExecutor", "<", "R", ">"),),
        "post-merge exact-output V2EffectExecutor::complete_application", errors)
    _require_rust_token_sequence(
        paths["effects"], complete_apply,
        """let pending = self.pending_applications.get(&completion.work_id)
.expect("the pending Apply was checked above");
self.preflight_pending_application_owner(completion.work_id, pending)""",
        "runtime Apply completion must preflight its exact separately retained owner before mutation",
        errors)
    _require_rust_token_sequence(
        paths["worker"], observer,
        """super::status::set_v2_effect_completion_observer(
self.context.id(), self.context.height, &io.admission, );
activation.complete(); Ok(())""",
        "completion observer activation must publish the live worker owner before completing its permit transaction",
        errors)

    lane_ctor = method("lane", "V2LaneWorkAdapter", "new_with_output_guard_and_transport_inner")
    lane_defer = method("lane", "V2LaneWorkAdapter", "defer_missing_recovered_decision_apply_sidecar")
    lane_accept = method("lane", "V2LaneWorkAdapter", "accept_certified_merge_sidecar_chunk")
    _require_rust_token_sequence(
        paths["lane"], lane_ctor,
        """recovered_apply_sidecar_waits: BTreeSet::new(),
rejected_recovered_apply_sidecars: BTreeMap::new(),""",
        "lane construction must initialize distinct recovered Apply wait and rejection owners", errors)
    _require_rust_token_sequence(
        paths["lane"], lane_defer,
        """MergeSidecarDeferralDisposition::Fetching
| MergeSidecarDeferralDisposition::RetryLater => { self.recovered_apply_sidecar_waits.insert(entry_hash); }
MergeSidecarDeferralDisposition::Available
| MergeSidecarDeferralDisposition::Rejected(_) => { self.recovered_apply_sidecar_waits.remove(&entry_hash); }""",
        "recovered Apply sidecar deferral must retain only live wait ownership", errors)
    _require_rust_token_sequence(
        paths["lane"], lane_accept,
        """if self.recovered_apply_sidecar_waits.remove(&entry_hash) {
self.rejected_recovered_apply_sidecars.entry(entry_hash).or_insert(error); }""",
        "invalid recovered Apply sidecars must move their wait into the dedicated rejection owner",
        errors, count=1)
    _require_rust_token_sequence(
        paths["lane"], lane_accept,
        """if self.recovered_apply_sidecar_waits.remove(&entry_hash) {
self.rejected_recovered_apply_sidecars.entry(entry_hash).or_insert(reason); }""",
        "globally invalid recovered Apply sidecars must move their wait into the dedicated rejection owner",
        errors, count=1)

    ordinary = method("merge", "MergeSidecarTransport", "defer_decided_block")
    lifecycle = method("merge", "MergeSidecarTransport", "defer_lifecycle_decided_block")
    register = method("merge", "MergeSidecarTransport", "defer_block_with_priority")
    retain = method("merge", "MergeSidecarTransport", "retain_pending_blocks")
    _require_rust_token_sequence(
        paths["merge"], ordinary,
        """InboundPriority::Decided, false,""",
        "ordinary decided sidecars must remain executor-census owned", errors)
    _require_rust_token_sequence(
        paths["merge"], lifecycle,
        """InboundPriority::Decided, true,""",
        "recovered Apply sidecars must enter the lifecycle-owned corridor", errors)
    _require_rust_token_sequence(
        paths["merge"], register,
        """.entry(block_hash).and_modify(|carrier| {
if lifecycle_owned { carrier.lifecycle_owned = true; }
}).or_insert(DeferredCarrier { hash: block_hash, height, view, lifecycle_owned, });""",
        "repeated exact registration must monotonically promote lifecycle ownership", errors)
    _require_rust_token_sequence(
        paths["merge"], retain,
        """carrier.height <= committed_height
|| (!carrier.lifecycle_owned && !pending_blocks.contains(&carrier.hash))""",
        "cleanup preflight must preserve live lifecycle-owned carriers while recognizing committed height",
        errors)
    _require_rust_token_sequence(
        paths["merge"], retain,
        """carrier.height > committed_height
&& (carrier.lifecycle_owned || pending_blocks.contains(hash))""",
        "cleanup must retain lifecycle-owned carriers only until their height commits", errors)
    return errors
