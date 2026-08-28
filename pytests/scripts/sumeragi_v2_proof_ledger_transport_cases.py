# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


def test_merge_sidecar_holder_semantics_survive_item_digest_refresh(
    tmp_path: Path,
) -> None:
    """Resealing cannot hide weakened certified-sidecar custody authority."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    lane_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    item_name = "service_next_certified_merge_sidecar_materialization"
    context = (("impl", "V2LaneWorkAdapter"),)
    mutate_rust_item_source_in_context(
        module,
        lane_path,
        item_name,
        context,
        "|| preferred_merge_sidecar_holder(&self.context, &reference)",
        "&& preferred_merge_sidecar_holder(&self.context, &reference)",
    )
    qualified = f"V2LaneWorkAdapter::{item_name}"
    original = rebind_reviewed_rust_item_digests(
        module,
        lane_path,
        item_name,
        context,
        ((module._PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256, qualified),),
    )
    try:
        errors = module._transport_hardening_production_source_fidelity_errors(
            tmp_path
        )
    finally:
        restore_reviewed_rust_item_digests(original)

    assert any(
        "local QC-holder or exact proposal-leader custody" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_delivery_binding",
            "&& self.delivery_binding.delivery_ordinal == self.delivery_ordinal",
            "&& true",
            "reply-route validation rejects any substituted owner, ordinal, target, or minting tenure",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "peer_message",
            "NetworkReplyRoute::new(origin.clone(), tenure, delivery_ordinal)",
            "NetworkReplyRoute { semantic_target: origin.clone(), tenure, delivery_ordinal, delivery_binding: unreachable!() }",
            "authenticated local delivery mints the immutable binding through the reviewed constructor with one checked actor-global ordinal",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_freshness_from",
            "self.validate_delivery_binding()?;",
            "let _unchecked = &self.delivery_binding;",
            "immutable source freshness validates bindings and rejects foreign, retargeted, or cross-source capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "try_from_route",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "route-set construction validates the actor-minted binding before importing a live capability",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "strict route-set preflight validates every retained and candidate delivery binding",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "single-route attachment validates the actor-minted binding before live-route admission",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_retired_delivery",
            "retired.validate_delivery_binding()?;",
            "let _unchecked = &retired.delivery_binding;",
            "candidate tombstones validate authority and can release a live source only through the joint tenure/delivery freshness kernel",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "reliable_progress_class",
            "return Some(ReliableProgressClass::Safety);",
            "return Some(ReliableProgressClass::Lane);",
            "public reliable-progress classes exactly refine actor reservations",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_key",
            "authenticated_via: self.tenure.delivery_peer.clone(),",
            "authenticated_via: self.semantic_target.clone(),",
            "reply fairness keys bind actor identity and authenticated delivery peer",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_freshness_from",
            "std::cmp::Ordering::Less => NetworkReplyRouteSourceFreshness::Stale,",
            "std::cmp::Ordering::Less => NetworkReplyRouteSourceFreshness::LaterDelivery,",
            "source freshness requires both tenure and delivery ordinals to increase on reconnect and rejects both collision classes",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "equal_ordinal_different_tenure",
            "&& !self.same_tenure(other)",
            "&& true",
            (
                "equal actor-global ordinals cannot be replayed under another "
                "connection tenure"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retain_active_with_receipt_after_snapshot",
            ".filter(|(_, route)| !route.is_active())",
            ".filter(|(_, _route)| false)",
            "owned route pruning removes only the exact inactive snapshot capability and binds its before/after receipt",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retain_active_with_receipt_after_snapshot",
            "self.record_retired_delivery(retired);",
            "drop(retired);",
            "owned route pruning removes only the exact inactive snapshot capability and binds its before/after receipt",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            ".any(|retired| retired.equal_ordinal_different_tenure(route))",
            ".any(|_retired| false)",
            "retired route history rejects both forged ordinal classes and non-progressing same-source replay",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            "self.retired_attempts.get(&route.source_key())",
            "None",
            "retired route history rejects both forged ordinal classes and non-progressing same-source replay",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_with_receipt",
            "for retired in candidate.retired_attempts.values().cloned() {\n"
            "            merged.merge_retired_delivery(retired)?;\n"
            "        }",
            "let _ = &candidate.retired_attempts;",
            "strict route-set merge preflights, applies tombstones before live siblings, and binds one exact transition receipt",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "retired.source_freshness_from(current),",
            "current.source_freshness_from(&retired),",
            "retired route history remains source-bounded and advances only through the joint tenure/delivery freshness kernel",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "if self.retired_attempts.len() >= self.source_capacity",
            "if false && self.retired_attempts.len() >= self.source_capacity",
            "retired route history remains source-bounded and advances only through the joint tenure/delivery freshness kernel",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|prior| prior.equal_ordinal_different_tenure(route))",
            ".any(|_prior| false)",
            "strict route-set preflight validates every member and rejects delivery- or connection-ordinal tenure collisions before mutation",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|(_, other)| route.equal_ordinal_different_tenure(other))",
            ".any(|(_, _other)| false)",
            "strict route-set preflight rejects internal delivery- or connection-ordinal tenure collisions atomically",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_retired_delivery",
            ".is_some_and(|freshness| !matches!(freshness, NetworkReplyRouteSourceFreshness::Stale));",
            ".is_some_and(|freshness| matches!(freshness, NetworkReplyRouteSourceFreshness::Stale));",
            "candidate tombstones validate authority and can release a live source only through the joint tenure/delivery freshness kernel",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            ".any(|prior| prior.equal_ordinal_different_tenure(&route))",
            ".any(|_prior| false)",
            "single-route attachment rejects delivery- and connection-ordinal reuse under different tenures",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_freshness_from",
            "if !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "if false && !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "immutable source freshness validates bindings and rejects foreign, retargeted, or cross-source capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_freshness_from",
            "return Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure);",
            "return Err(NetworkReplyRouteError::Stale);",
            "source freshness requires both tenure and delivery ordinals to increase on reconnect and rejects both collision classes",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            "if self.attempts.len() >= self.source_capacity {",
            "if false && self.attempts.len() >= self.source_capacity {",
            (
                "one source update tombstones only its prior delivery and a new "
                "source consumes one bounded attempt"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_with_receipt",
            "let mut merged = self.clone();",
            "let mut merged = candidate.clone();",
            "strict route-set merge preflights, applies tombstones before live siblings, and binds one exact transition receipt",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_from_transport_with_reply_route",
            "if reply_route.semantic_target() != &sender {",
            "if false && reply_route.semantic_target() != &sender {",
            "transport reply authority must bind both semantic target and independently authenticated hop",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_from_transport_with_reply_route",
            "return Err(NetworkReplyRouteError::DifferentSource);",
            "return Err(NetworkReplyRouteError::Retargeted);",
            "transport reply authority must bind both semantic target and independently authenticated hop",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "transport_reply_route_construction_is_fallible_and_target_bound",
            "Err(NetworkReplyRouteError::DifferentSource)",
            "Err(NetworkReplyRouteError::Retargeted)",
            "authoritative transport reply-route regression must match exact reviewed token digest",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_push_at",
            "merged.merge_with_receipt(candidate)",
            "merged.merge_with_receipt(retained)",
            "coalesced ingress shadow-merges route capacity and attempt cursors without mutating the retained owner",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "dequeue_selected_locked",
            "state.pending_wire_owners.remove(key)",
            "state.pending_wire_owners.get(key).cloned()",
            "semantic request ownership retires only when its queued occurrence is serviced",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "post_reply_recoverable_with_flush_ack_inner",
            "|| reply_route.validate_delivery_binding().is_err()",
            "|| false",
            "reply admission rejects retargeted, foreign-actor, or substituted delivery capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "post_reply_recoverable_with_flush_ack_inner",
            "target: Some(reply_route.tenure.delivery_peer.clone()),",
            "target: Some(reply_route.semantic_target().clone()),",
            (
                "reply admission accounts by authenticated delivery peer and "
                "transfers its exact timeout attempt and flush sender"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "release_cancelled_targets",
            "entry.publish_ready_exact_reply_before_terminal_drop();",
            "let _unfenced = &entry;",
            (
                "authority cancellation terminal-fences exact reply receipts before "
                "releasing target deliveries and scheduler membership"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_reply_route",
            "ProgressAuthorityIdentity::Reply(tenure.connection_ordinal),",
            "ProgressAuthorityIdentity::Reply(tenure.connection_ordinal.saturating_add(1)),",
            "reply-tenure cancellation selects only the authenticated source, exact connection authority",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_authority_waiters",
            "target: Some(source_peer.clone()),",
            "target: None,",
            "waiter cancellation is isolated to one authenticated source, progress class, delivery kind, and exact authority",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_all_reply_route_tenures",
            "tenure.cancel();",
            "let _uncancelled = &tenure;",
            "actor teardown atomically takes only its own tenure map, retires each exact tenure",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "run",
            "let released_on_shutdown = safety_dispatch_pending\n"
            "            .release_all_with_terminal_fence()\n"
            "            .saturating_add(progress_dispatch_pending.release_all_with_terminal_fence());",
            "let released_on_shutdown = safety_dispatch_pending\n"
            "            .release_cancelled_targets()\n"
            "            .saturating_add(progress_dispatch_pending.release_cancelled_targets());",
            (
                "normal actor exit terminal-fences pending exact receipts and publishes "
                "route cancellation before terminating or aborting peer writers"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "finish_reply_route_tenure",
            "tenure.cancel();",
            "let _uncancelled = &tenure;",
            (
                "receiver drain completion revokes the exact delivery tenure and "
                "clears its termination fence"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "dispatch_reliable_actor_message_inner",
            "if !current_writer || !current_tenure {",
            "if !current_writer && !current_tenure {",
            (
                "reply dispatch terminal-fences a ready old occurrence before retiring "
                "substituted writer tenure"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "reattach_reply_route",
            "if self.reply_route.is_some()",
            "if false",
            (
                "peer-message reply-route reattachment rejects capability overwrite, "
                "retargeted authority, or retired authority"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_reply_route_mutants(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    if item_name == "source_key" and relative == Path(
        "crates/iroha_p2p/src/network.rs"
    ):
        mutate_rust_item_source_in_context(
            module,
            repo_root / relative,
            item_name,
            (("impl", "NetworkReplyRoute"),),
            old,
            new,
        )
    else:
        mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "brace_context", "old", "new", "expected_error"),
    (
        (
            "new",
            (("impl", "NetworkReplyRoute"),),
            "owner: Arc::clone(&tenure.owner),",
            "owner: Arc::new(()),",
            "reply-route construction mints one immutable binding from the exact actor owner",
        ),
        (
            "new",
            (("impl", "NetworkReplyRoute"),),
            "minting_tenure: Arc::downgrade(&tenure),",
            "minting_tenure: Weak::new(),",
            "reply-route construction mints one immutable binding from the exact actor owner",
        ),
        (
            "drop",
            (
                (
                    "impl", "<", "T", ":", "Pload", ",", "E", ":", "Enc", ">",
                    "Drop", "for", "NetworkBase", "<", "T", ",", "E", ">",
                ),
            ),
            "let _ = self.cancel_all_reply_route_tenures();",
            "let _uncancelled = &self.reply_route_tenures;",
            "network actor Drop reuses the centralized idempotent reply-tenure teardown",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_contextual_route_mutants(
    tmp_path: Path,
    item_name: str,
    brace_context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates/iroha_p2p/src/network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        item_name,
        brace_context,
        old,
        new,
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "pub fn is_authenticated_via(&self, peer: &PeerId) -> bool {\n"
            "        &self.tenure.delivery_peer == peer\n"
            "    }",
            "pub(crate) fn is_authenticated_via(&self, peer: &PeerId) -> bool {\n"
            "        &self.tenure.delivery_peer == peer\n"
            "    }",
            "public opaque authenticated-hop binding must remain public",
        ),
        (
            "&self.tenure.delivery_peer == peer",
            "&self.tenure.delivery_peer != peer",
            "public opaque authenticated-hop binding must match exact reviewed token digest",
        ),
        (
            "pub fn merge_observed(&mut self, candidate: &Self)",
            "fn merge_observed(&mut self, candidate: &Self)",
            "public atomic observed-history reconciliation must remain public",
        ),
        (
            "minting_tenure: Weak<ReliableReplyRouteTenure>,",
            "minting_tenure: Arc<ReliableReplyRouteTenure>,",
            "reply delivery occurrences retain an immutable actor, minting-tenure, semantic-target, and actor-global ordinal binding",
        ),
        (
            "delivery_binding: Arc<ReliableReplyDeliveryBinding>,",
            "delivery_binding: Option<Arc<ReliableReplyDeliveryBinding>>,",
            "opaque reply routes carry their immutable actor-minted delivery binding beside the selected tenure",
        ),
    ),
)
def test_transport_geometry_source_fidelity_binds_public_route_helpers(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / "crates/iroha_p2p/src/network.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            ".release_retired_tenure_binding(peer_id, conn_id);",
            ".clear_generation(peer_id, conn_id);",
            "retired connection-generation-era clear_generation API must remain absent",
        ),
        (
            "retain_after_dispatch_attempt",
            "from_dispatch_parts",
            "retired reconstruction-era from_dispatch_parts API must remain absent",
        ),
        (
            "retired tenure binding lets a later live tenure service the same exact\n"
            "    /// frame without manufacturing or updating a reply capability.",
            "retired tenure binding means a successor session must be allowed to\n"
            "    /// reconstruct delivery instead of treating it as a stale duplicate.",
            "retired connection-generation-era wording",
        ),
        (
            "requester's\n"
            "    /// durable source retains its exact retry state throughout.",
            "requester's durable retry is its reconstruction path",
            "retired connection-generation-era wording",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_retired_generation_terminology(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / "crates/iroha_p2p/src/network.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count(old) >= 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "post_reply_recoverable",
            "self.post_reply_recoverable_with_flush_ack(msg, reply_route, ticket)\n"
            "            .map(|flush_ack| {\n"
            "                flush_ack.map_or(\n"
            "                    NetworkReplyAdmissionOutcome::ReplyWriterUnavailable,\n"
            "                    |_flush_ack| NetworkReplyAdmissionOutcome::Admitted,\n"
            "                )\n"
            "            })",
            "self.post_reply_recoverable_with_flush_ack(msg, reply_route, ticket)\n"
            "            .map(|_flush_ack| NetworkReplyAdmissionOutcome::Admitted)",
            (
                "reply admission distinguishes transferred writer ownership from a "
                "delivery-active but unwritable route"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack",
            "self.post_reply_recoverable_with_flush_ack_at_attempt("
            "msg, reply_route, ticket, 0)",
            "Ok(None)",
            (
                "public reply completion admission delegates at attempt zero without "
                "bypassing the shared preflight and budget path"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack_inner",
            "Some(reply_writer_timeout_attempt),\n"
            "            Some(reply_flush_sender),",
            "None,\n"
            "            Some(reply_flush_sender),",
            (
                "reply admission accounts by authenticated delivery peer and "
                "transfers its exact timeout attempt and flush sender"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack_inner",
            "Some(reply_writer_timeout_attempt),\n"
            "            Some(reply_flush_sender),",
            "Some(reply_writer_timeout_attempt),\n"
            "None,",
            (
                "reply admission accounts by authenticated delivery peer and "
                "transfers its exact timeout attempt and flush sender"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack_inner",
            "let identity = NetworkReplyFlushIdentity::from_admitted_ticket(ticket)\n"
            "                    .expect(\"validated reply admission must retain its exact reply shape\");\n"
            "                NetworkReplyFlushAck::new(identity, reply_flush_receiver)",
            "let identity = NetworkReplyFlushIdentity::from_admitted_ticket(ticket)\n"
            "                    .expect(\"validated reply admission must retain its exact reply shape\");\n"
            "                NetworkReplyFlushAck::new(forged_identity, reply_flush_receiver)",
            (
                "only a newly admitted exact ticket yields its immutable reply identity "
                "and live flush completion"
            ),
        ),
        (
            "submit_progress_message_to_source",
            "AdmittedNetworkMessage::new_targeted_post(\n"
            "                message,\n"
            "                lease,\n"
            "                authority,\n"
            "                reply_writer_timeout_attempt,\n"
            "                reply_flush_ack,\n"
            "            )",
            "AdmittedNetworkMessage::new_targeted_post(\n"
            "                message,\n"
            "                lease,\n"
            "                authority,\n"
            "                None,\n"
            "                reply_flush_ack,\n"
            "            )",
            (
                "accepted direct replies transfer their exact timeout attempt and flush "
                "sender while broadcasts cannot impersonate either"
            ),
        ),
        (
            "broadcast_recoverable",
            "target.actor_ticket.take(),\n"
            "                None,\n"
            "                None,",
            "target.actor_ticket.take(),\n"
            "                Some(0),\n"
            "                None,",
            (
                "broadcast fanout admits each active topology authority through an "
                "isolated target source without a timeout attempt or reply completion"
            ),
        ),
        (
            "into_dispatch_parts",
            "pending_flush_acks,\n"
            "            progress_authority,\n"
            "            reply_writer_timeout_attempt,\n"
            "            reply_writer_deadline,\n"
            "            reply_flush_ack,\n"
            "        )",
            "pending_flush_acks,\n"
            "            progress_authority,\n"
            "            None,\n"
            "            reply_writer_deadline,\n"
            "            reply_flush_ack,\n"
            "        )",
            (
                "dispatch tuple exports timeout attempt, fixed deadline, and exact reply "
                "completion sender without dropping them"
            ),
        ),
        (
            "retain_after_dispatch_attempt",
            "pending_flush_acks,\n"
            "            reply_writer_timeout_attempt,\n"
            "            reply_writer_deadline,\n"
            "            reply_flush_ack,",
            "pending_flush_acks,\n"
            "            reply_writer_timeout_attempt: None,\n"
            "            reply_writer_deadline,\n"
            "            reply_flush_ack,",
            (
                "incomplete dispatch retains timeout attempt, fixed deadline, and exact "
                "reply completion sender without reconstructing authority"
            ),
        ),
        (
            "dispatch_reliable_actor_message_inner",
            "if transferred {",
            "if true {",
            (
                "typed actor reply completion succeeds only after all exact writer "
                "flushes while timeout attempt and fixed deadline survive every retry"
            ),
        ),
        (
            "dispatch_reliable_actor_message_inner",
            "pending_flush_acks,\n"
            "                progress_authority,\n"
            "                reply_writer_timeout_attempt,\n"
            "                reply_writer_deadline,\n"
            "                reply_flush_ack,\n"
            "            ))",
            "pending_flush_acks,\n"
            "                progress_authority,\n"
            "                None,\n"
            "                reply_writer_deadline,\n"
            "                reply_flush_ack,\n"
            "            ))",
            (
                "typed actor reply completion succeeds only after all exact writer "
                "flushes while timeout attempt and fixed deadline survive every retry"
            ),
        ),
        (
            "poll",
            "self.terminal = Some(NetworkReplyFlushAckStatus::Closed);",
            "self.terminal = Some(NetworkReplyFlushAckStatus::Flushed);",
            (
                "typed writer completion keeps successful flush, explicit timeout, and "
                "ordinary closure distinct"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_reply_flush_ack_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "new",
            (("impl", "NetworkReplyFlushAck"),),
            "identity,\n            receiver: Some(receiver),\n            terminal: None,",
            "receiver: Some(receiver),\n            terminal: None,",
            (
                "new reply completion starts pending with the exact admitted identity "
                "and actor-owned receiver"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "new_targeted_post",
            (("impl", "<", "T", ">", "AdmittedNetworkMessage", "<", "T", ">"),),
            "pending_flush_acks: HashMap::new(),\n"
            "            reply_writer_timeout_attempt,\n"
            "            reply_writer_deadline: None,\n"
            "            reply_flush_ack,",
            "pending_flush_acks: HashMap::new(),\n"
            "            reply_writer_timeout_attempt: None,\n"
            "            reply_writer_deadline: None,\n"
            "            reply_flush_ack,",
            (
                "targeted actor-post construction keeps timeout identity, deadline state, "
                "and reply completion beside its exact lease and authority"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "push_back",
            (
                (
                    "impl",
                    "<",
                    "T",
                    ":",
                    "message",
                    "::",
                    "ClassifyTopic",
                    ">",
                    "ReliableActorPending",
                    "<",
                    "T",
                    ">",
                ),
            ),
            "entries.push_back(message);",
            "drop(message);",
            (
                "actor backlog insertion preserves the complete admitted owner "
                "under its exact source"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retry_back",
            (
                (
                    "impl",
                    "<",
                    "T",
                    ":",
                    "message",
                    "::",
                    "ClassifyTopic",
                    ">",
                    "ReliableActorPending",
                    "<",
                    "T",
                    ">",
                ),
            ),
            "self.push_back(message);",
            "drop(message);",
            "actor retry preserves the same source and opaque completion-bearing owner",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "defer_high_priority_network_message",
            (),
            "if sender.send(message).await.is_err() {",
            "drop(message);\n        if false {",
            (
                "deferred actor admission moves the complete opaque message "
                "owner into its bounded task"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "new",
            (("impl", "OutboundPostOwnership"),),
            "_byte_lease: byte_lease,\n            flush_ack,",
            "_byte_lease: byte_lease,\n            flush_ack: None,",
            (
                "peer-writer ownership constructor keeps the byte lease and "
                "optional flush sender inseparable"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "new",
            (("impl", "<", "T", ">", "RetainedPost", "<", "T", ">"),),
            "message: Some(message),\n            ownership,",
            "message: Some(message),\n            ownership: panic!(),",
            (
                "peer mailbox constructor retains the exact message beside its "
                "writer ownership"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "into_parts",
            (("impl", "<", "T", ">", "RetainedPost", "<", "T", ">"),),
            "ownership,\n        )",
            "panic!(),\n        )",
            (
                "peer mailbox extraction returns the exact message and writer "
                "ownership together"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "prepare_owned_or_defer",
            (
                ("mod", "run"),
                ("impl", "<", "E", ":", "Enc", ">", "MessageSender", "<", "E", ">"),
            ),
            "vec![ownership.into()]",
            "Vec::new()",
            (
                "peer plaintext admission transfers the exact mailbox owner into "
                "one writer-owned vector"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "retry_deferred",
            (
                ("mod", "run"),
                ("impl", "<", "E", ":", "Enc", ">", "MessageSender", "<", "E", ">"),
            ),
            "let scratch_ownership = "
            "core::mem::replace(&mut self.buffer_ownership, ownership);",
            "let scratch_ownership = "
            "core::mem::replace(&mut self.buffer_ownership, Vec::new());",
            "deferred retry restores its exact bytes and flush owners into the encoder",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "send_one_ready_stream",
            (("mod", "run"),),
            "(true, false) => Some((false, high.send().await)),",
            "(true, false) => Some((false, Ok(()))),",
            "single ready peer stream still enters the reviewed writer-flush kernel",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "next_peer_stream_io",
            (("mod", "run"),),
            "let send = send_one_ready_stream(high_sender, low_sender, prefer_low_send);",
            "let send = async { Some((false, Ok(()))) };",
            (
                "full-duplex peer IO delegates its outbound branch to the "
                "reviewed ready-stream writer"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "run",
            (("mod", "run"),),
            "stream_io = next_peer_stream_io(",
            "stream_io = unreviewed_peer_stream_io(",
            (
                "peer task routes ready writer work through the reviewed "
                "full-duplex IO seam"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_flush_owner_handoff_mutants(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    mutate_rust_item_source_in_context(
        module,
        repo_root / relative,
        item_name,
        context,
        old,
        new,
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_source_fidelity_rejects_success_signalling_owner_drop(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    source = peer_path.read_text(encoding="utf-8")
    marker = "impl From<SharedByteLease> for OutboundPostOwnership {"
    assert source.count(marker) == 1
    mutant = """
impl Drop for OutboundPostOwnership {
    fn drop(&mut self) {
        if let Some(flush_ack) = self.flush_ack.take() {
            let _ = flush_ack.send(());
        }
    }
}

"""
    peer_path.write_text(source.replace(marker, mutant + marker), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "OutboundPostOwnership must close a flush witness by ordinary sender drop"
        in error
        for error in errors
    ), errors


def test_transport_geometry_source_fidelity_rejects_progress_lease_drop_digest_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    baseline_errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    p2p_taira_markers = ("/crates/iroha_p2p/", "/configs/soranexus/taira/")
    assert not [
        error for error in baseline_errors
        if any(marker in error for marker in p2p_taira_markers)
    ], baseline_errors

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    source = network_path.read_text(encoding="utf-8")
    region_start = source.index("impl Drop for NetworkActorProgressLease")
    mutation = source.index(
        "retained.request_digest, self.request_digest,", region_start
    )
    old = "retained.request_digest, self.request_digest,"
    new = "self.request_digest, self.request_digest,"
    network_path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "progress lease drop releases only the same digest, kind, and delivery authority"
        in error
        for error in errors
    ), errors

    # Exercise every H/R split seam in one additional copied workspace so the
    # fixed proof-fidelity test count does not grow with the mutation matrix.
    geometry_formal_dir = copy_async_source_fidelity_fixture(
        tmp_path / "h_geometry", module, "SumeragiV2AsyncNetwork.tla"
    )
    geometry_root = geometry_formal_dir.parents[2]
    core_path = geometry_root / "crates/iroha_core/src/sumeragi/mod.rs"
    core_source = core_path.read_text(encoding="utf-8")
    core_source = core_source.replace(
        "    Authenticated(PeerId),\n}",
        "    Authenticated,\n}",
        1,
    )
    core_path.write_text(core_source, encoding="utf-8")
    mutate_rust_item_source(
        module,
        core_path,
        "fair_v2_ingress_required_capacity",
        "authenticated_non_validator_source_capacity\n"
        "            .checked_mul(3)",
        "authenticated_non_validator_source_capacity\n"
        "            .checked_mul(2)",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "fair_v2_ingress_required_byte_capacity",
        ".checked_add(authenticated_non_validator_source_capacity.unwrap_or(0))",
        ".checked_add(0)",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "try_push_at",
        "let source_lane_is_new = !state.lanes.contains_key(&source);",
        "let source_lane_is_new = false;",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "try_push_at",
        "let retained_authenticated_non_validator_sources = state\n"
        "                .lanes\n"
        "                .keys()\n"
        "                .filter(|source| matches!(source, FairV2IngressSource::Authenticated(_)))\n"
        "                .count();",
        "let retained_authenticated_non_validator_sources = state\n"
        "                .lanes\n"
        "                .keys()\n"
        "                .count();",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "dequeue_selected_locked",
        "} else if matches!(&source, FairV2IngressSource::Authenticated(_)) {",
        "} else if false && matches!(&source, FairV2IngressSource::Authenticated(_)) {",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "start",
        "let authenticated_non_validator_source_capacity =\n"
        "            config.queues.authenticated_non_validator_sources.get();",
        "let authenticated_non_validator_source_capacity =\n"
        "            network.reply_route_source_capacity();",
    )

    defaults_path = geometry_root / "crates/iroha_config/src/parameters/defaults.rs"
    defaults_source = defaults_path.read_text(encoding="utf-8")
    defaults_source = defaults_source.replace(
        "3 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
        "2 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
        1,
    )
    defaults_path.write_text(defaults_source, encoding="utf-8")

    actual_path = geometry_root / "crates/iroha_config/src/parameters/actual.rs"
    actual_source = actual_path.read_text(encoding="utf-8")
    actual_source = actual_source.replace(
        "body_queue_capacity,\n                "
        "authenticated_non_validator_source_capacity,\n                body_bytes,",
        "body_queue_capacity,\n                "
        "authenticated_non_validator_source_capacity: 0,\n                body_bytes,",
        1,
    )
    actual_path.write_text(actual_source, encoding="utf-8")

    user_path = geometry_root / "crates/iroha_config/src/parameters/user.rs"
    user_source = user_path.read_text(encoding="utf-8")
    user_source = user_source.replace(
        "sumeragi.queues.authenticated_non_validator_sources.get() "
        "> reply_source_capacity",
        "sumeragi.queues.authenticated_non_validator_sources.get() "
        ">= reply_source_capacity",
        1,
    )
    user_source = user_source.replace(
        ".or(lane_profile.derived_limits().max_total_connections)",
        ".or(None)",
        1,
    )
    user_path.write_text(user_source, encoding="utf-8")

    kagami_path = geometry_root / "crates/iroha_kagami/src/localnet.rs"
    mutate_rust_item_source(
        module,
        kagami_path,
        "localnet_sumeragi_body_bytes",
        "actual::sumeragi_v2_body_ingress_required_byte_capacity(\n"
        "        validator_count,\n"
        "        LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES,",
        "actual::sumeragi_v2_body_ingress_required_byte_capacity(\n"
        "        validator_count,\n"
        "        0,",
    )

    for relative in (Path("configs/soranexus/taira/config.toml"),):
        path = geometry_root / relative
        source = path.read_text(encoding="utf-8")
        path.write_text(
            source.replace("authenticated_non_validator_sources = 2", "", 1),
            encoding="utf-8",
        )
    geometry_errors = module._transport_geometry_production_source_fidelity_errors(
        geometry_root
    )
    for expected_error in (
        "two-way authenticated fair-ingress source ownership inventory",
        "semantic duplicate coalescing must precede new-lane admission",
        "authenticated non-validator lane cap excludes validator lanes",
        "empty authenticated non-validator lanes release their bounded churn slot",
        "exact default 5N+3H outer-ingress message geometry",
        "production H comes from Sumeragi ingress configuration rather than reply-route R",
        "root configuration derives R from the effective explicit or lane-profile network geometry",
        "root configuration rejects H greater than exact-output reply-source R",
        "shared Sumeragi fingerprint projection carries H beside ingress capacities",
        "localnet aggregate bytes scale by N+H",
        "production Taira profile pins H=2 and six source partitions",
    ):
        assert any(expected_error in error for error in geometry_errors), (
            expected_error,
            geometry_errors,
        )

    refresh_mutations = (
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "relay_message_wire_payload_len",
            "let origin_signature_len = byte_sequence_wire_len(origin_signature_bytes)?;",
            "let origin_signature_len = 0;",
            "relay geometry must charge the origin signature and every exact wire field",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "parse_next_encrypted_frame",
            ".reserve_decode_scratch(size)",
            ".reserve_decode_scratch(0)",
            "receiver decode scratch must be reserved before taking the source-owned ciphertext lease",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "handle_service_message",
            "self.soranet_transport_key_pair.clone(),",
            "self.key_pair.clone(),",
            "inbound stream handoff must carry separate validated node and transport identities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "handle_service_message",
            "self.network_id.clone(),",
            "test_network_id(\"foreign-network\"),",
            "inbound stream handoff must carry separate validated node and transport identities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "run",
            "self.set_validator_dial_roster(roster);",
            "let _ = roster;",
            "network actor must consume coupled validator dial-roster and topology ownership updates",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "peer_connected",
            "self.validator_dial_scheduler.note_session_established(",
            "unreviewed_validator_dial_scheduler.note_session_established(",
            "accepted peer must publish validator dial session ownership",
        ),
        (
            Path("configs/soranexus/taira/config.toml"),
            None,
            "max_frame_bytes = 23068700",
            "max_frame_bytes = 23068699",
            "production Taira profile carries maximum privacy transaction and block-sync frames",
        ),
        (
            Path("configs/soranexus/taira/genesis.json"),
            None,
            '"max_payload_size_bytes":16777216',
            '"max_payload_size_bytes":16777215',
            "production Taira genesis DA pins the revision-4 protocol ceiling",
        ),
    )
    refresh_source_relatives = (
        ("p2p_network", Path("crates/iroha_p2p/src/network.rs")),
        ("p2p_peer", Path("crates/iroha_p2p/src/peer.rs")),
        ("taira_config", Path("configs/soranexus/taira/config.toml")),
        ("taira_genesis", Path("configs/soranexus/taira/genesis.json")),
    )
    for index, (relative, item_name, old, new, expected_error) in enumerate(
        refresh_mutations
    ):
        mutation_formal_dir = copy_async_source_fidelity_fixture(
            tmp_path / f"refresh_{index}", module, "SumeragiV2AsyncNetwork.tla"
        )
        mutation_root = mutation_formal_dir.parents[2]
        path = mutation_root / relative
        if item_name is None:
            mutate_source_once(path, old, new)
        else:
            mutate_rust_item_source(module, path, item_name, old, new)

        mutation_paths = {
            role: mutation_root / source_relative
            for role, source_relative in refresh_source_relatives
        }
        mutation_sources = {
            role: source_path.read_text(encoding="utf-8")
            for role, source_path in mutation_paths.items()
        }
        mutation_errors = module._transport_geometry_refresh_resistant_errors(
            mutation_paths, mutation_sources
        )
        assert any(expected_error in error for error in mutation_errors), (
            expected_error,
            mutation_errors,
        )


def test_transport_geometry_source_fidelity_rejects_taira_semantic_mutants(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    markers = ("/crates/iroha_p2p/", "/configs/soranexus/taira/")
    baseline = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert not [error for error in baseline if any(marker in error for marker in markers)], baseline

    production_path = repo_root / "configs/soranexus/taira/genesis.json"
    production_source = production_path.read_text(encoding="utf-8")
    production_path.write_text(
        production_source.replace('"max_payload_size_bytes":16777216', '"max_payload_size_bytes":16777215', 1)
        .replace('"max_tx_bytes": 10485760', '"max_tx_bytes": 10485759', 1),
        encoding="utf-8",
    )
    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    for expected in (
        "production Taira genesis DA pins the revision-4 protocol ceiling",
        "production Taira genesis admits one maximum privacy transaction",
    ):
        assert any(expected in error for error in errors), (expected, errors)


def test_transport_geometry_source_fidelity_rejects_sm_distid_bit_length_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    sm_path = repo_root / "crates" / "iroha_crypto" / "src" / "sm.rs"
    mutate_rust_item_source(
        module,
        sm_path,
        "validate_distid",
        ".checked_mul(8)",
        ".checked_mul(1)",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "SM2 distinguishing-identifier geometry validate_distid" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new"),
    (
        (
            "fair_v2_ingress_required_capacity",
            ".checked_mul(3)",
            ".checked_mul(2)",
        ),
        (
            "fair_v2_ingress_lane_protected_slots",
            "3_usize.saturating_sub(depth)",
            "2_usize.saturating_sub(depth)",
        ),
        (
            "fair_v2_ingress_required_manifest_bytes",
            ".checked_add(228)",
            ".checked_add(227)",
        ),
        (
            "fair_v2_ingress_required_quorum_certificate_bytes",
            ".checked_add(fair_v2_ingress_framed_bytes(signer_vector_bytes)?)?",
            ".checked_add(0)?",
        ),
        (
            "fair_v2_ingress_required_proposal_bytes",
            "let timeout_group_vector_bytes = roster_len\n"
            "            .checked_mul(framed_timeout_group_bytes)?",
            "let timeout_group_vector_bytes = framed_timeout_group_bytes",
        ),
        (
            "fair_v2_ingress_required_p2p_frame_bytes",
            "Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES)",
            "Some(32)",
        ),
        (
            "fair_v2_ingress_required_recovery_request_bytes_for_key",
            "Some(certified_body_request.max(commit_certificate_request))",
            "Some(commit_certificate_request)",
        ),
        (
            "fair_v2_ingress_required_commit_certificate_response_bytes_for_key",
            ".checked_add(responder_bytes)?",
            ".checked_add(0)?",
        ),
        (
            "fair_v2_ingress_required_transport_completion_bytes",
            ".checked_add(fair_v2_ingress_framed_bytes(encoded_body_bytes)?)?",
            ".checked_add(encoded_body_bytes)?",
        ),
        (
            "configure_roster_for_context",
            "required_proposal_bytes.max(required_commit_certificate_response_bytes)",
            "required_proposal_bytes",
        ),
        (
            "configure_roster_for_context",
            ".max(required_recovery_request_bytes)",
            "",
        ),
        (
            "configure_roster_for_context",
            "required: usize::MAX,",
            "required: 0,",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_exact_progress_bound(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    mutate_rust_item_source(module, core_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "authoritative fair-v2 ingress geometry" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "configure_roster_for_context",
            ".max(required_lane_progress_frame_bytes)\n"
            "                .max(crate::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES);",
            ".max(required_lane_progress_frame_bytes);",
            "consensus frame geometry must include the exact Kura replica-advert network ceiling",
        ),
        (
            "configure_roster_for_context",
            ".max(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES)\n"
            "                .max(crate::MAX_KURA_REPLICA_ADVERT_NETWORK_FRAME_BYTES),",
            ".max(MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES),",
            "ordinary source geometry must include the exact Kura replica-advert network ceiling",
        ),
        (
            "configure_roster_for_context",
            "fair_v2_ingress_required_recovery_request_bytes(network_id, roster.len())",
            "fair_v2_ingress_required_recovery_request_bytes(&NetworkId::default(), roster.len())",
            "exact v2 and lane-local progress/completion/recovery ceilings",
        ),
        (
            "try_push_at",
            "queued.ownership_snapshot = ownership_snapshot;",
            "let _ = ownership_snapshot;",
            "validated ingress route shadow commits atomically beside its exact ownership evidence",
        ),
        (
            "try_push_at",
            "&& !authenticated_historical_recovery_response",
            "&& false",
            "current-roster or proof-carrying historical authority premise",
        ),
        (
            "dequeue_selected_locked",
            "state.pending_wire_owners.remove(key)",
            "state.pending_wire_owners.get(key).cloned()",
            "semantic request ownership retires only when its queued occurrence is serviced",
        ),
        (
            "try_recv_if_at_checked",
            "self.try_recv_if_at_checked_classified(service_attempt_at, false, predicate)",
            "self.try_recv_if_at_checked_classified(service_attempt_at, true, predicate)",
            "ordinary timestamped ingress must delegate to the single classifier",
        ),
    ),
)
def test_transport_geometry_reviewed_ingress_items_survive_digest_refresh(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Reviewed Kura geometry and ordinary selection survive an item reseal."""

    module = load_checker()
    formal_names = tuple(
        dict.fromkeys(
            (
                "SumeragiV2AsyncNetwork.tla",
                *module._TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256,
                *module._TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256,
            )
        )
    )
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, *formal_names
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates/iroha_core/src/sumeragi/mod.rs"
    mutate_rust_item_source_in_context(
        module,
        core_path,
        item_name,
        (("impl", "FairV2Ingress"),),
        old,
        new,
    )
    item = next(
        candidate
        for candidate in module.rust_items(
            core_path.read_text(encoding="utf-8"), item_name
        )
        if candidate.brace_context == (("impl", "FairV2Ingress"),)
    )
    digest = module._rust_item_token_sha256(item)
    module._PRODUCTION_FAIR_V2_INGRESS_IMPL_ITEM_SHA256[item_name] = digest
    if item_name == "try_recv_if_at_checked":
        module._LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[item_name] = digest
        module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256[
            f"ingress::{item_name}"
        ] = digest

    geometry_errors = (
        module._transport_geometry_production_source_fidelity_errors(repo_root)
    )
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in geometry_errors
    ), geometry_errors
    if item_name == "try_recv_if_at_checked":
        leader_errors = (
            module._leader_wire_physical_ingress_production_source_fidelity_errors(
                repo_root
            )
        )
        timeout_errors = module._timeout_vote_episode_source_fidelity_errors(
            repo_root, formal_dir
        )
        assert any(
            expected_error in error
            and "exact reviewed token digest" not in error
            for error in leader_errors
        ), leader_errors
        assert any(
            "ordinary timestamped ingress must use the same classifier "
            "without a bypass policy" in error
            and "exact reviewed token digest" not in error
            for error in timeout_errors
        ), timeout_errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "seal", "expected_error"),
    (
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "fair_v2_ingress_queue_gate_verdict",
            "if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass)",
            "if false || (!ingress_barrier_allows && !dependency_bypass)",
            "ingress::fair_v2_ingress_queue_gate_verdict",
            "live control predecessor blocks strict-newer carrier authorization",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
            "scheduler_arbitration_inputs",
            "(deferred_owner_blocks_fifo || older_signer_blocks_fifo)",
            "deferred_owner_blocks_fifo",
            "scheduler_arbitration_inputs",
            "deferred-owner alias or older-signer predecessor",
        ),
    ),
)
def test_core_runtime_moved_helper_semantics_survive_digest_refresh(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    seal: str,
    expected_error: str,
) -> None:
    """Moved selector and runtime blockers remain semantic after resealing."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = reviewed_rust_item_provider(module, repo_root, relative, item_name)
    mutate_rust_item_source(module, path, item_name, old, new)
    if item_name == "fair_v2_ingress_queue_gate_verdict":
        mutate_rust_item_source(
            module,
            path,
            item_name,
            "height != 0 && height < owner.token.identity.height",
            "height < owner.token.identity.height",
        )
        core_path = repo_root / relative
        for history_item, history_old, history_new in (
            (
                "matches_configured_network",
                "configured_network_id == Some(&network_id)",
                "configured_network_id != Some(&network_id)",
            ),
            (
                "fair_v2_ingress_history_serve_request",
                "v2_transport::authenticate_commit_certificate_request_identity(\n"
                "                request,\n"
                "                inbound.sender(),\n"
                "            )\n"
                "            .is_ok()",
                "v2_transport::authenticate_commit_certificate_request_identity(\n"
                "                request,\n"
                "                inbound.sender(),\n"
                "            )\n"
                "            .is_err()",
            ),
            (
                "new_with_source_geometry_and_transport_frame_caps",
                "configured_network_id: None,",
                "configured_network_id: Some(NetworkId::default()),",
            ),
            (
                "configure_roster_for_context",
                "roster,\n            Some(*network_id),",
                "roster,\n            None,",
            ),
            (
                "configure_roster_with_byte_requirements",
                "state.configured_network_id = configured_network_id;",
                "state.configured_network_id = None;",
            ),
            (
                "try_push_at",
                "request.matches_configured_network("
                "state.configured_network_id.as_ref())",
                "true",
            ),
        ):
            mutate_rust_item_source(
                module,
                core_path,
                history_item,
                history_old,
                history_new,
            )
        core_source = core_path.read_text(encoding="utf-8")
        for history_item in (
            "new_with_source_geometry_and_transport_frame_caps",
            "configure_roster_for_context",
            "configure_roster_with_byte_requirements",
            "try_push_at",
        ):
            history_item_source = next(
                candidate
                for candidate in module.rust_items(core_source, history_item)
                if candidate.brace_context == (("impl", "FairV2Ingress"),)
            )
            module._PRODUCTION_FAIR_V2_INGRESS_IMPL_ITEM_SHA256[
                history_item
            ] = module._rust_item_token_sha256(history_item_source)
        history_network = next(
            candidate
            for candidate in module.rust_items(
                core_source, "matches_configured_network"
            )
            if candidate.brace_context
            == (("impl", "FairV2IngressHistoryServeRequest"),)
        )
        module._PRODUCTION_FAIR_V2_INGRESS_HISTORY_ITEM_SHA256[
            "FairV2IngressHistoryServeRequest::matches_configured_network"
        ] = module._rust_item_token_sha256(history_network)
        history_request = module.rust_items(
            core_source, "fair_v2_ingress_history_serve_request"
        )[0]
        module._PRODUCTION_FAIR_V2_INGRESS_HISTORY_ITEM_SHA256[
            "fair_v2_ingress_history_serve_request"
        ] = module._rust_item_token_sha256(history_request)
    item = module.rust_items(path.read_text(encoding="utf-8"), item_name)[0]
    digest = module._rust_item_token_sha256(item)
    if seal.startswith("ingress::"):
        module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256[seal] = digest
        module._LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[item_name] = digest
        module._PRODUCTION_FAIR_V2_INGRESS_TOP_LEVEL_ITEM_SHA256[item_name] = digest
    else:
        module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256[seal] = digest

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors
    if item_name == "fair_v2_ingress_queue_gate_verdict":
        assert any(
            "historical replica release must require a nonzero older request"
            in error
            and "exact reviewed token digest" not in error
            for error in errors
        ), errors
        for expected_history_error in (
            "an unconfigured ingress must begin closed and without network",
            "production context setup must atomically hand off the exact",
            "roster rollover must close admission, retire prior owners",
            "historical scheduling authority must be filtered against the exact",
            "only exact signed body or commit-certificate requests may acquire",
            "a signed request network must exactly equal the active",
        ):
            assert any(
                expected_history_error in error
                and "exact reviewed token digest" not in error
                for error in errors
            ), (expected_history_error, errors)


@pytest.mark.parametrize(
    ("item_name", "context", "old", "new", "expected_error"),
    (
        (
            "classify",
            (("impl", "FairV2IngressClass"),),
            "Self::classify_message(inbound.message())",
            "Self::Auxiliary",
            "authoritative fair-v2 ingress geometry classify",
        ),
        (
            "try_push_at",
            (("impl", "FairV2Ingress"),),
            "&& !authenticated_historical_recovery_response",
            "&& false",
            "current-roster or proof-carrying historical authority premise",
        ),
        (
            "dequeue_selected_locked",
            (("impl", "FairV2Ingress"),),
            "if entry.class == FairV2IngressClass::TransportCompletion {",
            "if false && entry.class == FairV2IngressClass::TransportCompletion {",
            "exact shared transport-completion owner retirement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_completion_owner_mutants(
    tmp_path: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    mutate_rust_item_source_in_context(
        module, core_path, item_name, context, old, new
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "required_field", "capacity_field", "kind"),
    tuple(
        (item_name, required_field, capacity_field, kind)
        for item_name in ("configure_roster_with_byte_requirements", "open")
        for required_field, capacity_field, kind in (
            (
                "required_consensus_frame_bytes",
                "consensus_frame_byte_capacity",
                "ConsensusFrameBytes",
            ),
            (
                "required_control_frame_bytes",
                "control_frame_byte_capacity",
                "ControlFrameBytes",
            ),
            (
                "required_block_sync_frame_bytes",
                "block_sync_frame_byte_capacity",
                "BlockSyncFrameBytes",
            ),
            (
                "required_outbound_high_frame_bytes",
                "outbound_high_frame_byte_capacity",
                "OutboundHighFrameBytes",
            ),
        )
    ),
)
def test_transport_geometry_source_fidelity_requires_configure_and_open_rechecks(
    tmp_path: Path,
    item_name: str,
    required_field: str,
    capacity_field: str,
    kind: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    guard = f"""        if state.{required_field} > self.{capacity_field} {{
            return Err(FairV2IngressCapacityError {{
                configured: self.{capacity_field},
                required: state.{required_field},
                kind: FairV2IngressCapacityKind::{kind},
            }});
        }}
"""
    mutate_rust_item_source(module, core_path, item_name, guard, "")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    stage = "configure" if item_name.startswith("configure") else "open"
    assert any(f"{stage} recheck for {kind}" in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            "iroha_p2p::frame_plaintext_cap(max_frame_bytes)",
            "usize::MAX",
            "encrypted global and three plaintext progress-topic cap intersection",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            ".min(max_frame_bytes_control)",
            ".min(usize::MAX)",
            "encrypted global and three plaintext progress-topic cap intersection",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            "block_sync_frame_byte_capacity,\n"
            "                outbound_frame_queue_max_high_bytes,",
            "block_sync_frame_byte_capacity,\n                usize::MAX,",
            "production fair-ingress construction with configured H and every progress cap",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "start_with_runtime_deps",
            "max_frame_bytes: config.network.max_frame_bytes,",
            "max_frame_bytes: usize::MAX,",
            "daemon-to-Sumeragi global/topic/high-queue cap hand-off",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "start_with_crypto_and_initial_authorities",
            "let transport_geometry = validate_transport_queue_geometry::<E>(",
            "let transport_geometry = unchecked_transport_queue_geometry::<E>(",
            "complete transport geometry validation must be the first P2P startup action before any listener bind",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "start_with_crypto",
            "Self::start_with_crypto_and_initial_trusted_sources(",
            "Self::unchecked_startup(",
            "public P2P crypto startup wrapper delegates through protected-source startup",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "validate_config",
            "validate_network_frame_runtime_limit(config)?;",
            "let _ = config;",
            "validate_config deterministic frame ceiling before IO/runtime probes",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "validate_config_offline",
            "validate_network_frame_runtime_limit(config)?;",
            "let _ = config;",
            "validate_config_offline deterministic frame ceiling before IO/runtime probes",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "start_with_runtime_deps",
            ".p2p_outbound_frame_queue_max_high_bytes\n                .get(),",
            ".p2p_outbound_frame_queue_max_low_bytes\n                .get(),",
            "daemon-to-Sumeragi global/topic/high-queue cap hand-off",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_startup_cap_bypass(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "nonzero!(128 * 1024 * 1024_usize)",
            "nonzero!(16 * 1024 * 1024_usize)",
            "high-priority encrypted-frame byte reserve",
        ),
        (
            "nonzero!(17 * 1024 * 1024_usize)",
            "nonzero!(16 * 1024 * 1024_usize)",
            "encrypted global consensus frame ceiling",
        ),
        (
            "pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = "
            "MAX_PLAINTEXT_FRAME_BYTES;",
            "pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = "
            "MAX_FRAME_BYTES_CONTROL;",
            "consensus-recovery frame ceiling",
        ),
        (
            "nonzero!(2 * 1024 * 1024_usize)",
            "nonzero!(1024 * 1024_usize)",
            "consensus-safety frame ceiling",
        ),
        (
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = "
            "MAX_PLAINTEXT_FRAME_BYTES;",
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = "
            "MAX_FRAME_BYTES_CONTROL;",
            "payload-completion frame ceiling",
        ),
        (
            "nonzero!(\n"
            "        5 * MAX_VALIDATORS_PER_HEIGHT + 3 * "
            "QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "    )",
            "nonzero!(\n"
            "        4 * MAX_VALIDATORS_PER_HEIGHT + 3 * "
            "QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "    )",
            "exact default 5N+3H outer-ingress message geometry",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_shortened_default_cap(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    defaults_path = (
        repo_root / "crates" / "iroha_config" / "src" / "parameters" / "defaults.rs"
    )
    source = defaults_path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    defaults_path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "pub const MAX_WIRE_ENCRYPTED_FRAME_BYTES: usize = u32::MAX as usize;",
            "pub const MAX_WIRE_ENCRYPTED_FRAME_BYTES: usize = u16::MAX as usize;",
            "exact u32 encrypted-frame wire-body ceiling",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "pub const MAX_ENCRYPTED_FRAME_BYTES: usize = 2_147_483_643;",
            "pub const MAX_ENCRYPTED_FRAME_BYTES: usize = u32::MAX as usize;",
            "deterministic cross-platform encrypted-frame runtime ceiling",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "if max_frame_bytes > crate::MAX_ENCRYPTED_FRAME_BYTES {",
            "if max_frame_bytes >= crate::MAX_ENCRYPTED_FRAME_BYTES {",
            "inclusive deterministic encrypted-frame runtime limit",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "if configured > iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES {",
            "if configured >= iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES {",
            "inclusive daemon deterministic encrypted-frame runtime limit",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            ".min(crate::MAX_ENCRYPTED_FRAME_BYTES)\n"
            "            .saturating_sub(core::mem::size_of::<aead::Nonce<E>>())",
            ".min(usize::MAX)\n"
            "            .saturating_sub(core::mem::size_of::<aead::Nonce<E>>())",
            "generic AEAD P2P preflight frame_plaintext_cap_for",
        ),
        (
            Path("crates/iroha_crypto/src/lib.rs"),
            "pub const MAX_PUBLIC_KEY_PAYLOAD_BYTES: usize = "
            "2 + (u16::MAX as usize / 8) + 65;",
            "pub const MAX_PUBLIC_KEY_PAYLOAD_BYTES: usize = 32;",
            "protocol-wide maximum public-key payload geometry",
        ),
        (
            Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
            "pub const MAX_VALIDATORS_PER_HEIGHT: usize = 3 * MAX_FAULTS_PER_HEIGHT + 1;",
            "pub const MAX_VALIDATORS_PER_HEIGHT: usize = 3 * MAX_FAULTS_PER_HEIGHT;",
            "first-release maximum validator geometry",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "let size = buf.get_u32() as usize;\n"
            "                if size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "let size = buf.get_u32() as usize;\n"
            "                if size > self.max_frame_bytes {",
            "runtime-clamped receiver parse boundary",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_cap_threading_mutants(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / relative
    source = path.read_text(encoding="utf-8")
    assert old in source, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors




def test_exact_output_source_seals_reject_combined_ownership_mutations(
    tmp_path: Path,
) -> None:
    """Keep refreshed physical, lane, Kura, and runner ownership fail-closed."""

    module = load_checker()
    sources = (
        "crates/iroha_core/src/lib.rs",
        "crates/iroha_core/src/merge_sidecar.rs",
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "crates/iroha_core/src/sumeragi/v2_core.rs",
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "crates/iroha_config/src/parameters/defaults.rs",
        "crates/iroha_config/src/parameters/actual.rs",
        "crates/iroha_config/src/parameters/user.rs",
    )
    for relative in sources:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)

    mutations = (
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "runtime_physical_cut: Option<u128>,",
            "runtime_physical_cut: Option<u64>,",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker/exact_output_rollover_claim.rs",
            "HashOf::new(message) != message_hash",
            "HashOf::new(message) == message_hash",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            ".schedule_retired_exact_output_heights(",
            ".schedule_retired_exact_output_heights_unchecked(",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "certified_serve_ingress_binding.retire()?;",
            "let _ = certified_serve_ingress_binding.retire();",
        ),
    )
    for relative, old, new in mutations:
        path = tmp_path / relative
        source = path.read_text(encoding="utf-8")
        assert old in source, (relative, old)
        path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)
    expected_fragments = (
        "fair-ingress evidence must bind semantic occurrence history",
        "exact-output claim validate_non_retireable_lane_transport_fanout",
        "production handoff must pass exact lane and Kura authorities into retirement",
        "every clean shutdown and finality path must retire the exact Serve ingress binding",
    )
    for fragment in expected_fragments:
        assert any(fragment in error for error in errors), (fragment, errors)
