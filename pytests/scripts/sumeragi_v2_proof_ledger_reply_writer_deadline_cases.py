"""Reply-writer deadline production-fidelity cases executed by the parent suite."""

def test_reply_writer_deadline_production_source_is_bound() -> None:
    module = load_checker()

    assert (
        module._reply_writer_deadline_production_source_fidelity_errors(ROOT_DIR)
        == []
    )

def test_reply_writer_deadline_selects_flush_fixture_methods_by_context(
    tmp_path: Path,
) -> None:
    """Same-named admission-fixture helpers cannot shadow flush fixtures."""

    module = load_checker()
    copy_reply_writer_deadline_fixture(tmp_path)
    network = tmp_path / "crates/iroha_p2p/src/network.rs"
    source = network.read_text(encoding="utf-8")
    for item_name in ("for_reply", "for_reply_at_attempt"):
        candidates = module.rust_items(source, item_name)
        assert len(candidates) == 2
        assert (
            sum(
                item.brace_context == REPLY_FLUSH_FIXTURE_CONTEXT
                for item in candidates
            )
            == 1
        )

    canonical_errors = (
        module._reply_writer_deadline_production_source_fidelity_errors(
            tmp_path
        )
    )
    assert not any(
        "function item named" in error and "for_reply" in error
        for error in canonical_errors
    ), canonical_errors

    mutate_rust_item_source_in_context(
        module,
        network,
        "for_reply_at_attempt",
        REPLY_FLUSH_FIXTURE_CONTEXT,
        "reply_writer_timeout_attempt: Some(reply_writer_timeout_attempt),",
        "reply_writer_timeout_attempt: Some(0),",
    )
    errors = module._reply_writer_deadline_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        "attempt-aware test fixture must retain the requested timeout generation"
        in error
        for error in errors
    ), errors

    mutate_source_once(
        network,
        "pub async fn start_with_crypto_and_initial_authorities(",
        "pub async fn start_with_crypto_and_initial_authorities_disabled(",
    )
    errors = module._reply_writer_deadline_production_source_fidelity_errors(tmp_path)
    assert any(
        "P2P startup must destructure the configured exact-reply writer deadline" in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    ("relative_path", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const REPLY_WRITER_FLUSH_TIMEOUT: Duration = Duration::from_secs(30);",
            "pub const REPLY_WRITER_FLUSH_TIMEOUT: Duration = Duration::from_secs(0);",
            "must default to 30 seconds",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub reply_writer_flush_timeout: Duration,",
            "pub reply_writer_flush_timeout: Option<Duration>,",
            "actual network config must retain",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            '#[config(default = "defaults::network::REPLY_WRITER_FLUSH_TIMEOUT.into()")]\n'
            "    pub reply_writer_flush_timeout_ms: DurationMs,",
            '#[config(default = "defaults::network::IDLE_TIMEOUT.into()")]\n'
            "    pub reply_writer_flush_timeout_ms: DurationMs,",
            "must expose the exact-reply writer deadline with its production default",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "let reply_writer_flush_timeout = reply_writer_flush_timeout.get().max(min_interval);",
            "let reply_writer_flush_timeout = reply_writer_flush_timeout.get();",
            "must be clamped to the 100ms timer floor",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "struct PendingWriterFlush {\n"
            "    receiver: tokio::sync::oneshot::Receiver<()>,\n"
            "}",
            "struct PendingWriterFlush {\n"
            "    receiver: tokio::sync::oneshot::Receiver<()>,\n"
            "    deadline: tokio::time::Instant,\n"
            "}",
            "must retain only its flush receiver",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "struct ExactReplyWriterDeadline {\n"
            "    admitted_at: tokio::time::Instant,\n"
            "    timeout: Duration,\n"
            "}",
            "struct ExactReplyWriterDeadline {\n"
            "    admitted_at: tokio::time::Instant,\n"
            "}",
            "must retain its first-dispatch instant and scaled timeout",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "enum NetworkReplyFlushCompletion {\n"
            "    Flushed,\n"
            "    TimedOut,\n"
            "}",
            "enum NetworkReplyFlushCompletion {\n"
            "    Flushed,\n"
            "}",
            "may explicitly publish only successful flush or timeout",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "impl<T> Drop for ReliableActorPending<T> {\n"
            "    fn drop(&mut self) {\n"
            "        let _ = self.release_all_with_terminal_fence();\n"
            "    }\n"
            "}",
            "impl<T> Drop for ReliableActorPending<T> {\n"
            "    fn drop(&mut self) {\n"
            "        let _ = self.len;\n"
            "    }\n"
            "}",
            "pending-queue Drop must fence every retained exact occurrence",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "let released_on_shutdown = safety_dispatch_pending\n"
            "            .release_all_with_terminal_fence()\n"
            "            .saturating_add(progress_dispatch_pending.release_all_with_terminal_fence());",
            "let released_on_shutdown = safety_dispatch_pending.len()\n"
            "            .saturating_add(progress_dispatch_pending.len());",
            "graceful shutdown must fence local exact receivers",
        ),
    ),
)
def test_reply_writer_deadline_production_global_source_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    copy_reply_writer_deadline_fixture(tmp_path)
    mutate_source_once(tmp_path / relative_path, old, new)

    errors = module._reply_writer_deadline_production_source_fidelity_errors(
        tmp_path
    )

    assert any(error_fragment in error for error in errors), errors

@pytest.mark.parametrize(
    ("relative_path", "item_name", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_p2p/src/network.rs",
            "scaled_reply_writer_flush_timeout",
            "timeout.checked_mul(2)",
            "timeout.checked_mul(1)",
            "saturating adaptive reply-writer timeout scaler",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "from_admitted_ticket",
            "if ticket.shape.reply_writer_timeout_attempt.is_none()",
            "if false",
            "release-mode identity construction must reject a missing timeout attempt",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_ticket",
            "&& self.shape == other.shape",
            "&& true",
            "admitted ticket equality must include the complete attempt-bearing shape",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "try_reserve_for_source",
            "|| ticket.shape != shape",
            "|| false",
            "retry admission must reject a changed timeout-attempt-bearing shape",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "for_reply_at_attempt",
            "reply_writer_timeout_attempt: Some(reply_writer_timeout_attempt),",
            "reply_writer_timeout_attempt: Some(0),",
            "attempt-aware test fixture must retain the requested timeout generation",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "post_reply_recoverable_with_flush_ack_inner",
            "Some(reply_writer_timeout_attempt),\n"
            "            Some(reply_flush_sender),",
            "None,\n"
            "            Some(reply_flush_sender),",
            "production admission must bind the caller's timeout generation",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "reply_writer_deadline.get_or_insert_with",
            "reply_writer_deadline.insert",
            "first actor dispatch must acquire one fixed",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "ack_targets.sort();",
            "ack_targets.reverse();",
            "polled in deterministic order before route or timeout retirement",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "if exact_reply_flushed {",
            "if false {",
            "ready exact receipt must publish Flushed",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "let frame = RelayMessage::new_signed(\n"
            "                        &self.key_pair,\n"
            "                        RelayTarget::Direct(post.peer_id.clone()),\n"
            "                        self.relay_ttl,\n"
            "                        post.priority,",
            "let frame = RelayMessage::new(\n"
            "                        RelayTarget::Direct(post.peer_id.clone()),\n"
            "                        self.relay_ttl,\n"
            "                        post.priority,",
            "exact direct reply dispatch must retain the authenticated relay origin",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "let frame = RelayMessage::new_signed(\n"
            "                            &self.key_pair,\n"
            "                            RelayTarget::Broadcast,\n"
            "                            self.relay_ttl,\n"
            "                            broadcast.priority,",
            "let frame = RelayMessage::new(\n"
            "                            RelayTarget::Broadcast,\n"
            "                            self.relay_ttl,\n"
            "                            broadcast.priority,",
            "reliable broadcast dispatch must retain the authenticated relay origin",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "let timed_out_reply_writer = reply_route.is_some()",
            "let timed_out_reply_writer = reply_route.is_none()",
            "only an exact reply may expire",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "reply_flush_ack.send(NetworkReplyFlushCompletion::TimedOut)",
            "reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed)",
            "must publish TimedOut rather than fabricate a flush",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "exact_reply_flush_wins_terminal_fence",
            "pending.receiver.close();",
            "let _ = &pending.receiver;",
            "close-and-immediate-poll terminal fence",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "dispatch_reliable_actor_message_inner",
            "after_initial_flush_poll();",
            "let _ = after_initial_flush_poll;",
            "deterministic test seam must run exactly after",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "release_cancelled_targets",
            "entry.publish_ready_exact_reply_before_terminal_drop();",
            "let _ = &entry.pending_flush_acks;",
            "inactive pending cleanup must fence",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "release_all_with_terminal_fence",
            "entry.publish_ready_exact_reply_before_terminal_drop();",
            "let _ = &entry.pending_flush_acks;",
            "shutdown cleanup must fence every pending exact occurrence",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "accept_reliable_actor_message",
            "message.publish_ready_exact_reply_before_terminal_drop();",
            "let _ = &message.pending_flush_acks;",
            "early inactive-authority admission drop must use the terminal fence",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "expire_reply_writer_occurrence",
            "if route.tenure.connection_id != connection_id {",
            "if route.tenure.connection_id == connection_id {",
            "exact accepting-connection timeout retirement",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "poll",
            "Ok(NetworkReplyFlushCompletion::TimedOut) => {",
            "Ok(NetworkReplyFlushCompletion::Flushed) => {",
            "terminal reply-flush outcome classifier",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "new_targeted_broadcast",
            "reply_writer_deadline: None,",
            "reply_writer_deadline: Some(ExactReplyWriterDeadline {\n"
            "                admitted_at: tokio::time::Instant::now(),\n"
            "                timeout: Duration::ZERO,\n"
            "            }),",
            "topology actor-item constructor",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "retain_after_dispatch_attempt",
            "reply_writer_deadline,\n"
            "            reply_flush_ack,\n"
            "        }",
            "reply_writer_deadline: None,\n"
            "            reply_flush_ack,\n"
            "        }",
            "full-queue actor-item retention",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "poll_reply_flushes",
            "pending_flush.reply_writer_timeout_attempt != current_timeout_attempt",
            "pending_flush.reply_writer_timeout_attempt != 0",
            "terminal reply-flush polling must preserve one adaptive-attempt identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "drive_with_budget_ack",
            "|| flush_ack.identity().reply_writer_timeout_attempt()\n"
            "                            != reply_writer_timeout_attempt",
            "|| false",
            "ordinary reply installation must reject an acknowledgement from another timeout generation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "handoff_applied_height_to_durable_reconstruction",
            "|| pending_flush.reply_writer_timeout_attempt\n"
            "                            != target.reply_writer_timeout_attempt",
            "|| false",
            "finality handoff must preserve target, retained occurrence, and acknowledgement timeout-attempt identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "poll_reply_flushes",
            "if matches!(status, NetworkReplyFlushAckStatus::TimedOut) {",
            "if matches!(status, NetworkReplyFlushAckStatus::Closed) {",
            "only TimedOut may grow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "mark_admitted",
            "target.reply_writer_timeout_attempt = 0;",
            "target.reply_writer_timeout_attempt =\n"
            "            target.reply_writer_timeout_attempt.saturating_add(1);",
            "only successful cursor advance resets",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "ready_exact_reply_flush_wins_connection_replacement",
            "async fn ready_exact_reply_flush_wins_connection_replacement()",
            "async fn ready_exact_reply_flush_wins_connection_replacement_mutant()",
            "ready_exact_reply_flush_wins_connection_replacement",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "terminal_fence_observes_deadline_flush_published_after_initial_poll",
            "async fn terminal_fence_observes_deadline_flush_published_after_initial_poll()",
            "async fn terminal_fence_deadline_gap_mutant()",
            "terminal_fence_observes_deadline_flush_published_after_initial_poll",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "reply_flush_identity_requires_and_exposes_timeout_attempt",
            "fn reply_flush_identity_requires_and_exposes_timeout_attempt()",
            "fn reply_flush_identity_ignores_timeout_attempt_mutant()",
            "reply_flush_identity_requires_and_exposes_timeout_attempt",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "reply_flush_test_fixture_distinguishes_success_timeout_and_close",
            "fn reply_flush_test_fixture_distinguishes_success_timeout_and_close()",
            "fn reply_flush_test_fixture_merges_terminal_outcomes_mutant()",
            "reply_flush_test_fixture_distinguishes_success_timeout_and_close",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance",
            "fn reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance()",
            "fn reply_flush_attempt_identity_mismatch_advances_mutant()",
            "reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "sidecar_flush_admission_retains_timeout_attempt_identity",
            "fn sidecar_flush_admission_retains_timeout_attempt_identity()",
            "fn sidecar_flush_admission_drops_timeout_attempt_mutant()",
            "sidecar_flush_admission_retains_timeout_attempt_identity",
        ),
    ),
)
def test_reply_writer_deadline_production_item_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    item_name: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    copy_reply_writer_deadline_fixture(tmp_path)
    if (
        relative_path == "crates/iroha_p2p/src/network.rs"
        and item_name in {"for_reply", "for_reply_at_attempt"}
    ):
        mutate_rust_item_source_in_context(
            module,
            tmp_path / relative_path,
            item_name,
            REPLY_FLUSH_FIXTURE_CONTEXT,
            old,
            new,
        )
    else:
        mutate_rust_item_source(
            module,
            tmp_path / relative_path,
            item_name,
            old,
            new,
        )
    if relative_path == "crates/iroha_core/src/sumeragi/v2_worker.rs" and item_name == "drive_with_budget_ack":
        mutated = module.rust_items((tmp_path / relative_path).read_text(encoding="utf-8"), item_name)
        assert len(mutated) == 1
        module._REPLY_WRITER_DEADLINE_WORKER_ITEM_SHA256[
            "PendingExactOutput::drive_with_budget_ack"
        ] = module._rust_item_token_sha256(mutated[0])

    errors = module._reply_writer_deadline_production_source_fidelity_errors(
        tmp_path
    )

    assert any(error_fragment in error for error in errors), errors
