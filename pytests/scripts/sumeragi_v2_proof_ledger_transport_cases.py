# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

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
            "source_update_from",
            "self.validate_delivery_binding()?;",
            "let _unchecked = &self.delivery_binding;",
            "per-source updates validate both actor-minted delivery bindings before classifying rank",
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
            "candidate tombstones validate their immutable binding and authority",
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
            "source_update_from",
            "if self.delivery_ordinal < prior.delivery_ordinal {",
            "if false && self.delivery_ordinal < prior.delivery_ordinal {",
            "per-source delivery ordinals reject stale or forged equal-ordinal tenures",
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
            "retain_active",
            "self.attempts.retain(|_, route| route.is_active());",
            "self.attempts.retain(|_, _route| true);",
            (
                "owned route-set maintenance tombstones then releases only "
                "inactive connection tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retain_active",
            "self.record_retired_delivery(retired);",
            "drop(retired);",
            (
                "owned route-set maintenance tombstones then releases only "
                "inactive connection tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            ".any(|retired| retired.equal_ordinal_different_tenure(route))",
            ".any(|_retired| false)",
            (
                "retired route history rejects forged equal ordinals and "
                "non-progressing same-source replay"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            "self.retired_attempts.get(&route.source_key())",
            "None",
            (
                "retired route history rejects forged equal ordinals and "
                "non-progressing same-source replay"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge",
            "for retired in candidate.retired_attempts.values().cloned() {\n"
            "            merged.merge_retired_delivery(retired)?;\n"
            "        }",
            "let _ = &candidate.retired_attempts;",
            (
                "strict route-set merge preflights then applies tombstones "
                "before live siblings on one atomic shadow copy"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "if retired.delivery_ordinal > current.delivery_ordinal {",
            "if retired.delivery_ordinal < current.delivery_ordinal {",
            (
                "retired route history remains source-bounded and monotonic by "
                "actor-global delivery ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "if self.retired_attempts.len() >= self.source_capacity",
            "if false && self.retired_attempts.len() >= self.source_capacity",
            (
                "retired route history remains source-bounded and monotonic by "
                "actor-global delivery ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|prior| prior.equal_ordinal_different_tenure(route))",
            ".any(|_prior| false)",
            (
                "strict route-set preflight validates every live and "
                "tombstoned candidate member before mutation"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|(_, other)| route.equal_ordinal_different_tenure(other))",
            ".any(|(_, _other)| false)",
            (
                "strict route-set preflight rejects internal equal-ordinal "
                "tenure collisions atomically"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_retired_delivery",
            "retired.delivery_ordinal >= current.delivery_ordinal",
            "retired.delivery_ordinal < current.delivery_ordinal",
            (
                "candidate tombstones validate their immutable binding and authority and can release only "
                "a same-source live attempt at an equal or later ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            ".any(|prior| prior.equal_ordinal_different_tenure(&route))",
            ".any(|_prior| false)",
            (
                "single-route attachment rejects equal actor-global ordinals "
                "under different tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            "if !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "if false && !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "per-source updates reject inactive, foreign, retargeted, and cross-source capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            ".ok_or(NetworkReplyRouteError::EqualOrdinalDifferentTenure);",
            ".ok_or(NetworkReplyRouteError::Stale);",
            "per-source delivery ordinals reject stale or forged equal-ordinal tenures",
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
            "merge",
            "let mut merged = self.clone();",
            "let mut merged = candidate.clone();",
            (
                "strict route-set merge preflights then applies tombstones "
                "before live siblings on one atomic shadow copy"
            ),
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
            "coalesced ingress shadow-merges one source route without mutating the retained owner",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_recv_if_at_checked_classified",
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
            "pub fn is_authenticated_via(&self, peer: &PeerId) -> bool {",
            "pub(crate) fn is_authenticated_via(&self, peer: &PeerId) -> bool {",
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
            "requester's durable source retains its exact retry state.",
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
            "            .map(|flush_ack| match flush_ack {\n"
            "                Some(_flush_ack) => NetworkReplyAdmissionOutcome::Admitted,\n"
            "                None => NetworkReplyAdmissionOutcome::ReplyWriterUnavailable,\n"
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
    p2p_taira_markers = (
        "/crates/iroha_p2p/", "/scripts/render_taira_validator_bundle.py",
        "/configs/soranexus/taira/", "/defaults/kagami/iroha3-taira/",
    )
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
        "    Authenticated(PeerId),\n    Anonymous,",
        "    Authenticated,\n    Anonymous,",
        1,
    )
    core_path.write_text(core_source, encoding="utf-8")
    mutate_rust_item_source(
        module,
        core_path,
        "fair_v2_ingress_required_capacity",
        "authenticated_non_validator_source_capacity\n"
        "                .checked_mul(2)",
        "authenticated_non_validator_source_capacity\n"
        "                .checked_mul(1)",
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
        "try_recv_if_at_checked_classified",
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
        "+ 2 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
        "+ QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
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
        ".checked_add(LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES)",
        ".checked_add(0)",
    )

    renderer_path = geometry_root / "scripts/render_taira_validator_bundle.py"
    renderer_source = renderer_path.read_text(encoding="utf-8")
    renderer_source = renderer_source.replace(
        "validator_count + authenticated_non_validator_sources + 1",
        "validator_count + 1",
        1,
    )
    renderer_path.write_text(renderer_source, encoding="utf-8")

    for relative in (
        Path("defaults/kagami/iroha3-taira/config.toml"),
        Path("configs/soranexus/taira/config.toml"),
    ):
        path = geometry_root / relative
        source = path.read_text(encoding="utf-8")
        path.write_text(
            source.replace("authenticated_non_validator_sources = 2", "", 1),
            encoding="utf-8",
        )
    readme_path = geometry_root / "configs/soranexus/taira/README.md"
    readme_source = readme_path.read_text(encoding="utf-8")
    readme_path.write_text(
        readme_source.replace(
            "validator_count + authenticated_non_validator_sources + 1",
            "validator_count + 1",
            1,
        ),
        encoding="utf-8",
    )

    geometry_errors = module._transport_geometry_production_source_fidelity_errors(
        geometry_root
    )
    for expected_error in (
        "three-way fair-ingress source ownership inventory",
        "semantic duplicate route attachment precedes authenticated non-validator lane-cap admission",
        "authenticated non-validator lane cap excludes validator and anonymous lanes",
        "empty authenticated non-validator lanes release their bounded churn slot",
        "exact default 5N+3H+2 outer-ingress message geometry",
        "production H comes from Sumeragi ingress configuration rather than reply-route R",
        "root configuration derives R from the effective explicit or lane-profile network geometry",
        "root configuration rejects H greater than exact-output reply-source R",
        "shared Sumeragi fingerprint projection carries H beside ingress capacities",
        "localnet aggregate bytes scale by N+H+1",
        "Taira renderer scales aggregate bytes by N+H+1",
        "default seven-validator Taira profile pins H=2 and ten source partitions",
        "production Taira profile pins H=2 and seven source partitions",
        "Taira operator documentation states N+H+1 byte scaling",
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
            Path("defaults/kagami/iroha3-taira/config.toml"),
            None,
            "max_frame_bytes = 23068700",
            "max_frame_bytes = 23068699",
            "default Taira profile carries maximum privacy transaction and block-sync frames",
        ),
        (
            Path("defaults/kagami/iroha3-taira/genesis.json"),
            None,
            '"max_payload_size_bytes": 16777216',
            '"max_payload_size_bytes": 16777215',
            "default Taira genesis DA pins the revision-4 protocol ceiling",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            None,
            "const TAIRA_MAX_FRAME_BYTES: usize = 23_068_700;",
            "const TAIRA_MAX_FRAME_BYTES: usize = 23_068_699;",
            "Kagami Taira encrypted-frame ceiling",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            None,
            "const TAIRA_MAX_FRAME_BYTES_BLOCK_SYNC: usize = 23_068_672;",
            "const TAIRA_MAX_FRAME_BYTES_BLOCK_SYNC: usize = 23_068_671;",
            "Kagami Taira block-sync frame ceiling",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            None,
            "const TAIRA_MAX_FRAME_BYTES_TX_GOSSIP: usize = 11_534_336;",
            "const TAIRA_MAX_FRAME_BYTES_TX_GOSSIP: usize = 11_534_335;",
            "Kagami Taira transaction-gossip frame ceiling",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            "render_peer_config_with_private_keys",
            'let taira_network_frame_overrides = if spec.slug == "iroha3-taira" {',
            'let taira_network_frame_overrides = if spec.slug == "iroha3-dev" {',
            "Taira-only Kagami frame override branch",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            "render_peer_config_with_private_keys",
            "fn render_peer_config_with_private_keys(",
            "fn disabled_render_peer_config_with_private_keys(",
            "require exactly one Kagami render_peer_config_with_private_keys item",
        ),
        (
            Path("xtask/src/kagami_profiles.rs"),
            "render_peer_config_with_private_keys",
            "taira_network_frame_overrides = taira_network_frame_overrides,",
            "taira_network_frame_overrides = String::new(),",
            "install the Taira frame overrides in the network table",
        ),
    )
    refresh_source_relatives = (
        ("p2p_network", Path("crates/iroha_p2p/src/network.rs")),
        ("p2p_peer", Path("crates/iroha_p2p/src/peer.rs")),
        ("kagami_profiles", Path("xtask/src/kagami_profiles.rs")),
        ("taira_renderer", Path("scripts/render_taira_validator_bundle.py")),
        ("taira_default", Path("defaults/kagami/iroha3-taira/config.toml")),
        ("taira_config", Path("configs/soranexus/taira/config.toml")),
        ("taira_genesis", Path("configs/soranexus/taira/genesis.json")),
        (
            "taira_default_genesis",
            Path("defaults/kagami/iroha3-taira/genesis.json"),
        ),
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
    markers = (
        "/crates/iroha_p2p/", "/scripts/render_taira_validator_bundle.py",
        "/configs/soranexus/taira/", "/defaults/kagami/iroha3-taira/",
    )
    baseline = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert not [error for error in baseline if any(marker in error for marker in markers)], baseline

    renderer_path = repo_root / "scripts/render_taira_validator_bundle.py"
    renderer_source = renderer_path.read_text(encoding="utf-8")
    renderer_path.write_text(
        renderer_source.replace(
            "validator_count + authenticated_non_validator_sources + 1",
            "validator_count + 1",
            1,
        ),
        encoding="utf-8",
    )
    production_path = repo_root / "configs/soranexus/taira/genesis.json"
    production_source = production_path.read_text(encoding="utf-8")
    production_path.write_text(
        production_source.replace('"max_payload_size_bytes":16777216', '"max_payload_size_bytes":16777215', 1)
        .replace('"max_tx_bytes": 10485760', '"max_tx_bytes": 10485759', 1),
        encoding="utf-8",
    )
    default_path = repo_root / "defaults/kagami/iroha3-taira/genesis.json"
    default_source = default_path.read_text(encoding="utf-8")
    default_path.write_text(
        default_source.replace(
            '"max_payload_size_bytes": 16777216,',
            '"max_payload_size_bytes": 16777216,\n      "max_payload_size_bytes": 16777216,',
            1,
        ),
        encoding="utf-8",
    )
    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    for expected in (
        "Taira renderer scales aggregate bytes by N+H+1",
        "production Taira genesis DA pins the revision-4 protocol ceiling",
        "production Taira genesis admits one maximum privacy transaction",
        "default Taira genesis must parse with unique keys",
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
            ".checked_mul(4)",
            ".checked_mul(3)",
        ),
        (
            "fair_v2_ingress_lane_protected_slots",
            "4_usize.saturating_sub(depth)",
            "3_usize.saturating_sub(depth)",
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
            ".max(required_recovery_request_bytes),",
            ",",
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
            "FairV2IngressBarrierBypass::None,",
            "FairV2IngressBarrierBypass::TimeoutVoteEpisode,",
            "ordinary timestamped ingress must delegate with no barrier bypass",
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
            "ordinary timestamped ingress must pass "
            "FairV2IngressBarrierBypass::None" in error
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
    item = module.rust_items(path.read_text(encoding="utf-8"), item_name)[0]
    digest = module._rust_item_token_sha256(item)
    if seal.startswith("ingress::"):
        module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256[seal] = digest
    else:
        module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256[seal] = digest

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors


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
            "            outbound_frame_queue_max_high_bytes,",
            "block_sync_frame_byte_capacity,\n            usize::MAX,",
            "production fair-ingress construction with configured H and every progress cap",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "start",
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
            "start",
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
            "pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = MAX_FRAME_BYTES;",
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
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = MAX_FRAME_BYTES;",
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = "
            "MAX_FRAME_BYTES_CONTROL;",
            "payload-completion frame ceiling",
        ),
        (
            "nonzero!(\n"
            "        5 * MAX_VALIDATORS_PER_HEIGHT\n"
            "            + 3 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "            + 2\n"
            "    )",
            "nonzero!(\n"
            "        4 * MAX_VALIDATORS_PER_HEIGHT\n"
            "            + 3 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "            + 2\n"
            "    )",
            "exact default 5N+3H+2 outer-ingress message geometry",
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
            "            if size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "let size = buf.get_u32() as usize;\n"
            "            if size > self.max_frame_bytes {",
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
