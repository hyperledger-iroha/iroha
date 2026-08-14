_REPLY_WRITER_DEADLINE_NETWORK_ITEM_SHA256 = {
    "NetworkReplyFlushIdentity::from_admitted_ticket": (
        "f675d999da4ab51537f07fc3e02388d5f461d6cf358810aece18dbf173391b1a"
    ),
    "NetworkReplyFlushIdentity::reply_writer_timeout_attempt": (
        "9bb2ff1ad3102a493d96593046da0ee373d2e5899ccbb421c6b022fd95697d64"
    ),
    "NetworkReplyFlushAck::poll": (
        "512c90e2877329331a6ec24ae26eb1a6d021fe735cd42756def239d1e5101cd2"
    ),
    "ExactReplyWriterDeadline::expired_at": (
        "c389b204d86dc852b17b52d93e554756ff239e0267ae9897a5e1182a3d7ba2dd"
    ),
    "scaled_reply_writer_flush_timeout": (
        "859cafc2303b8ac91026360be2f147285ee488ed77b4ca24eedbf9eee6c8b5f5"
    ),
    "AdmittedNetworkMessage::new_targeted_broadcast": (
        "3400bf2b24a89a67dc01e63b5c0ef6d46b1664ffcd256b889bc555236ba523f5"
    ),
    "AdmittedNetworkMessage::new_targeted_post": (
        "cfaedf859e6596e63c2806c44afcca063f66ec1e660f75047e433ae5a30e727f"
    ),
    "AdmittedNetworkMessage::into_dispatch_parts": (
        "cca1a2804e76c9ba9d07319bf5256be6c53c2e0c39237ace52aa53b86a75a5c2"
    ),
    "AdmittedNetworkMessage::retain_after_dispatch_attempt": (
        "6dfa7ebe62b1a848b55c0ef43f945296b51f9014dfff44a1166bf042a6ca582b"
    ),
    "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_at_attempt": (
        "a805d671a5bd8724bfc297c403818a4b693f633d0a9f052aa94de22128d000e4"
    ),
    "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_inner": (
        "b3597f519695465158e058cd91b48617aaace25fa496a5f9f64833c010c84c8c"
    ),
    "NetworkReplyFlushAckTestFixture::for_reply": (
        "6957692044294dab904fef62dc0e4e021e7369e3cd4bcc2c200b8601e140d6fd"
    ),
    "NetworkReplyFlushAckTestFixture::for_reply_at_attempt": (
        "6876e8c9aecf1d9ae055b34625f51b29c088ff8f4266c3fd78a7f49dc1bea2d8"
    ),
    "exact_reply_flush_wins_terminal_fence": (
        "32831aa79d57dc0512bec6ad404d21e590baf75e818f158be9e727adb8a3b21a"
    ),
    "AdmittedNetworkMessage::publish_ready_exact_reply_before_terminal_drop": (
        "623a4ace191f99adb0b649f4105120990193a02120520c4172ed0fbf15eaf547"
    ),
    "ReliableActorPending::release_all_with_terminal_fence": (
        "bafe5b662de83b20bfb6c6e6c36e8b5106396a1be11657ff93089cdf78bcdd5f"
    ),
    "ReliableActorPending::release_cancelled_targets": (
        "88ad8d2a8a0183ec28c8baf59d40af851b135abd7c46af65ed411f9ef7fe5b4d"
    ),
    "NetworkBase::dispatch_reliable_actor_message": (
        "21cf34eb5aa68209a6baf29124170f073ea3a65be8c4e5887d6c5fbb69ffe20f"
    ),
    "NetworkBase::dispatch_reliable_actor_message_inner": (
        "3988f0adb7eae269376964cbe9bd11e6437b7c9955076523704ee7f9f6633687"
    ),
    "NetworkBase::accept_reliable_actor_message": (
        "93884415a18ff4e3f7c0c680d0219b8a1d01ebdca398b9c8534aa678a20a21e0"
    ),
    "NetworkBase::post_reliable_actor_frame_to_writer": (
        "9347732994ea2854fc8185768ca08bbd0b3bac410fdb65b3d724b4ccb71d0c4b"
    ),
    "NetworkBase::expire_reply_writer_occurrence": (
        "47b73c3e71be2a9896d208bb15097d7359194a738374bee812813a44d4f8a140"
    ),
}

_REPLY_WRITER_DEADLINE_NETWORK_TEST_SHA256 = {
    "reply_timeout_attempt_is_retained_by_actor_admission_ticket": (
        "0986ff63db60a1a90e682f162f160ed1d75e4f4897b6fc0eaa606a97ad788f1c"
    ),
    "reply_flush_identity_requires_and_exposes_timeout_attempt": (
        "082c9390e0720be8b3e6cb2ff96a12e1668a2987f35dd555072fa6ff06889689"
    ),
    "reply_flush_test_fixture_distinguishes_success_timeout_and_close": (
        "48ed5103779d849e1313f465b6c7035f93e79b638249e2c3fabe23de4535355f"
    ),
    "reply_flush_ack_completes_only_after_peer_writer_flush": (
        "3e9f44feed1f19b96c701d1619b3f47b76ce81c21dc925140c2c842f5faecfe5"
    ),
    "ready_exact_reply_flush_wins_route_retirement": (
        "6be7e1c0a3a76e1b2beeb053e221a3c4d39765b9921cbace30d28d9b10792bea"
    ),
    "ready_exact_reply_flush_wins_connection_replacement": (
        "1909afb3fb7b49b8b5b70c1e05ce476ea39211e812f3059c8c9121fc22f5b0d3"
    ),
    "terminal_fence_observes_deadline_flush_published_after_initial_poll": (
        "ee3d49954f7957baa856a48eb49719c5aec0780cb15c1013239d5735b461710e"
    ),
    "terminal_fence_observes_replacement_flush_published_after_initial_poll": (
        "80cd284419da2442153124fc630f237f39a998415ec02c856e856a5b28bd2ee2"
    ),
    "terminal_fence_observes_inactive_route_flush_published_after_initial_poll": (
        "0d92a63389e961f7b3c1b98245cc764064cd96341e2bb64be91713fa80aead68"
    ),
    "terminal_fence_observes_send_before_close_and_rejects_send_after_close": (
        "dca159634c2cc1debe1e1351d309aefa4789cc91e0dd608e3db8a288ecd0c72e"
    ),
    "cancelled_pending_exact_reply_observes_ready_flush_before_release": (
        "87c9bc8d641c6ce2368ac46cfa36ce4ffa9497ec1beee48f4e4b73a4c4d858ff"
    ),
    "pending_queue_drop_observes_ready_exact_flush_before_shutdown_close": (
        "4c3c103d23b777143d128315b42a818b5a1a8ec3cdee9c5176c0919a9488169a"
    ),
    "nonready_exact_reply_ack_cannot_keep_stale_route_alive": (
        "8c1524858f353d036644796393799834ca0ed3014d806d3954c92d30cf9b4c3f"
    ),
    "adaptive_reply_attempt_flushes_between_base_and_doubled_deadline": (
        "5385f9c6491f133e5209fcb0ffad4626ac839876ec9dc9ce30975727c1a2e808"
    ),
    "adaptive_reply_timeout_scaling_handles_extreme_duration_without_panicking": (
        "ce2e2978ca8d3e216f9e88d1b8e7d5dab753f90ad1b7d03347345a655735d078"
    ),
    "full_exact_writer_queue_times_out_closes_route_and_releases_actor_budget": (
        "27e08446be5cc78d25829edbcd87612707a7bfca92d6578136b3f80e4d74c722"
    ),
    "topology_writer_full_retry_does_not_acquire_exact_reply_deadline": (
        "160a0aebe147a211a085857454cf71867e7b8f7b5a0bdfed54ad65b6adb64a77"
    ),
    "stale_reply_writer_deadline_does_not_terminate_replacement": (
        "5d2281de1428b09b1b8543001e2de3db746fdaac3a35316257c1dc5e6bde7a98"
    ),
    "reply_writer_deadline_retirement_is_idempotent": (
        "0014e77c434b195ea916ff323ee5b70a609c1fe5219459c22bd3bd70fa793a7c"
    ),
}

_REPLY_WRITER_DEADLINE_WORKER_ITEM_SHA256 = {
    "PendingExactOutput::handoff_applied_height_to_durable_reconstruction": (
        "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80"
    ),
    "PendingExactOutput::drive_with_budget_ack": (
        "675d0e99a923f9b8cc96d725458584f4ea762cd4fd69fd9f8e7e76b158c82d52"
    ),
    "PendingExactOutput::poll_reply_flushes": (
        "eae8ee4dc4996b077b9d0e3315e96e8c35a18b0189f2add40e898e60a4167749"
    ),
    "PendingExactFanout::mark_admitted": (
        "c6e502433ef5249540446d75e0f88f665a7ffc456bf4014216f808fd123f072c"
    ),
    "PendingExactOutput::advance_after_attempt": (
        "e678eb75bf1e124b8c3ac7b196bc9abee8d5429a6f064a1aaf64851291bc0e07"
    ),
}

_REPLY_WRITER_DEADLINE_WORKER_TEST_SHA256 = {
    "ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses": (
        "9921c8135a2cae120a03e930d644f10ace06d9956d12eb64fad2c70ac6e9d027"
    ),
    "closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance": (
        "0f3fc3fc6668817adf3cf4092b6143fabe9a96dce60bf406c5d0d751c70cff7d"
    ),
    "adaptive_reply_timeout_grows_closed_preserves_and_flushed_resets_attempt": (
        "222096cb621c53d93789a76bfc8a4738cb39def2909c70027ff91a54e7b0a633"
    ),
    "reply_flush_attempt_identity_mismatch_fails_without_cursor_or_attempt_advance": (
        "3d955e108faa39cc4e5b7b16801e87093719fdd85ad4defb3015d9f46d5992db"
    ),
}

_REPLY_WRITER_DEADLINE_MERGE_TEST_SHA256 = {
    "sidecar_flush_admission_retains_timeout_attempt_identity": (
        "1ecbc2c924039bdf616a5de23771dc100b9885baef097467894f97c5060c2b1c"
    ),
}


def _reply_writer_deadline_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind exact-reply writer deadlines and their config path to production."""

    defaults_path = (
        repo_root
        / "crates"
        / "iroha_config"
        / "src"
        / "parameters"
        / "defaults.rs"
    )
    actual_path = (
        repo_root
        / "crates"
        / "iroha_config"
        / "src"
        / "parameters"
        / "actual.rs"
    )
    user_path = (
        repo_root
        / "crates"
        / "iroha_config"
        / "src"
        / "parameters"
        / "user.rs"
    )
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    merge_path = repo_root / "crates" / "iroha_core" / "src" / "merge_sidecar.rs"
    worker_path = (
        repo_root
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_worker.rs"
    )
    errors: list[str] = []
    sources: dict[Path, str] = {}
    for path, description in (
        (defaults_path, "reply-writer deadline default source"),
        (actual_path, "reply-writer deadline actual-config source"),
        (user_path, "reply-writer deadline user-config source"),
        (network_path, "exact reply peer-writer source"),
        (merge_path, "exact sidecar reply-attempt regression source"),
        (worker_path, "exact reply adaptive-attempt worker source"),
    ):
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: {description} must be a regular file")
            sources[path] = ""
        else:
            sources[path] = path.read_text(encoding="utf-8")

    _loaded_worker_path, reviewed_worker_source = _read_reviewed_rust_source(
        repo_root,
        worker_path.relative_to(repo_root).as_posix(),
        errors,
        "exact reply adaptive-attempt worker source",
    )
    if reviewed_worker_source:
        sources[worker_path] = reviewed_worker_source

    token_cache = {
        path: rust_code_tokens(source) for path, source in sources.items()
    }

    def require(
        path: Path,
        expected_source: str,
        description: str,
        *,
        count: int = 1,
    ) -> None:
        observed = _token_sequence_count(
            token_cache[path],
            rust_code_tokens(expected_source),
        )
        if observed != count:
            errors.append(
                f"{path}: {description} must occur exactly {count} time(s) in "
                f"executable Rust source; found {observed}"
            )

    require(
        defaults_path,
        """
pub const REPLY_WRITER_FLUSH_TIMEOUT: Duration = Duration::from_secs(30);
""",
        "the exact-reply writer deadline must default to 30 seconds",
    )
    require(
        actual_path,
        "pub reply_writer_flush_timeout: Duration,",
        "actual network config must retain the exact-reply writer deadline",
    )
    require(
        user_path,
        """
#[config(default = "defaults::network::REPLY_WRITER_FLUSH_TIMEOUT.into()")]
pub reply_writer_flush_timeout_ms: DurationMs,
""",
        "user network config must expose the exact-reply writer deadline with its production default",
    )
    reply_writer_default_literal = (
        '"defaults::network::REPLY_WRITER_FLUSH_TIMEOUT.into()"'
    )
    observed_reply_writer_default_literals = sources[user_path].count(
        reply_writer_default_literal
    )
    if observed_reply_writer_default_literals != 1:
        errors.append(
            f"{user_path}: user network config must expose the exact-reply "
            "writer deadline with its production default by binding the field "
            "to REPLY_WRITER_FLUSH_TIMEOUT exactly once; found "
            f"{observed_reply_writer_default_literals}"
        )
    require(
        user_path,
        """
let min_interval = MIN_TIMER_INTERVAL;
let idle_timeout = idle_timeout.get().max(min_interval);
let reply_writer_flush_timeout = reply_writer_flush_timeout.get().max(min_interval);
let dial_timeout = dial_timeout.get().max(min_interval);
""",
        "the configured exact-reply writer deadline must be clamped to the 100ms timer floor",
    )
    require(
        user_path,
        """
idle_timeout,
reply_writer_flush_timeout,
connect_startup_delay: connect_startup_delay.get(),
""",
        "user parsing must thread the clamped exact-reply writer deadline into actual network config",
    )

    require(
        network_path,
        """
struct PendingWriterFlush {
    receiver: tokio::sync::oneshot::Receiver<()>,
}
""",
        "a pending peer-writer occurrence must retain only its flush receiver; the deadline belongs to the actor item",
    )
    require(
        network_path,
        """
struct ExactReplyWriterDeadline {
    admitted_at: tokio::time::Instant,
    timeout: Duration,
}
""",
        "each exact-reply actor item must retain its first-dispatch instant and scaled timeout",
    )
    require(
        network_path,
        """
pub enum NetworkReplyFlushAckStatus {
    Pending,
    Flushed,
    TimedOut,
    Closed,
}
""",
        "reply completion must keep Pending, Flushed, TimedOut, and Closed distinct",
    )
    require(
        network_path,
        """
enum NetworkReplyFlushCompletion {
    Flushed,
    TimedOut,
}
""",
        "the actor may explicitly publish only successful flush or timeout",
    )
    require(
        network_path,
        """
reply_writer_timeout_attempt: Option<u8>,
reply_writer_deadline: Option<ExactReplyWriterDeadline>,
reply_flush_ack: Option<tokio::sync::oneshot::Sender<NetworkReplyFlushCompletion>>,
""",
        "the actor item must retain adaptive attempt, fixed deadline, and exact completion sender",
        count=2,
    )
    require(
        network_path,
        """
pub async fn start_with_crypto_and_initial_authorities(
    identity_keys: P2pIdentityKeys,
    Config {
        address: listen_addr,
        public_address,
        relay_mode,
        relay_hub_addresses,
        relay_ttl,
        soranet_handshake,
        idle_timeout,
        reply_writer_flush_timeout,
        connect_startup_delay,
""",
        "P2P startup must destructure the configured exact-reply writer deadline",
    )
    require(
        network_path,
        """
current_peers_addresses: Vec::new(),
idle_timeout,
reply_writer_flush_timeout,
dial_timeout,
connect_startup_delay_until,
""",
        "P2P startup must install the exact-reply writer deadline in the live actor",
    )

    network_source = sources[network_path]
    merge_source = sources[merge_path]
    worker_source = sources[worker_path]
    identity_context = (("impl", "NetworkReplyFlushIdentity"),)
    ack_context = (("impl", "NetworkReplyFlushAck"),)
    deadline_context = (("impl", "ExactReplyWriterDeadline"),)
    admitted_context = (
        ("impl", "<", "T", ">", "AdmittedNetworkMessage", "<", "T", ">"),
    )
    pending_context = (
        ("impl", "<", "T", ">", "ReliableActorPending", "<", "T", ">"),
    )
    classified_pending_context = (
        (
            "impl", "<", "T", ":", "message", "::", "ClassifyTopic", ">",
            "ReliableActorPending", "<", "T", ">",
        ),
    )
    handle_context = (
        (
            "impl", "<", "T", ":", "Pload", "+", "message", "::",
            "ClassifyTopic", ",", "E", ":", "Enc", "+", "Sync", ">",
            "NetworkBaseHandle", "<", "T", ",", "E", ">",
        ),
    )
    flush_fixture_context = (
        (
            "#", "[", "cfg", "(", "any", "(", "test", ",", "feature",
            "=", ")", ")", "]", "impl", "NetworkReplyFlushAckTestFixture",
        ),
    )
    actor_context = (
        (
            "impl", "<", "T", ":", "Pload", "+", "message", "::",
            "ClassifyTopic", ",", "E", ":", "Enc", ">", "NetworkBase",
            "<", "T", ",", "E", ">",
        ),
    )
    network_test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )
    worker_context = (("impl", "PendingExactOutput"),)
    fanout_context = (("impl", "PendingExactFanout"),)
    worker_test_context = (
        (
            "#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super",
            ")", "mod", "tests",
        ),
    )
    merge_test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )

    network_items: dict[str, RustItem | None] = {}
    item_contracts = (
        (
            "NetworkReplyFlushIdentity::from_admitted_ticket",
            "from_admitted_ticket",
            identity_context,
            (),
            "fail-closed admitted reply identity constructor",
        ),
        (
            "NetworkReplyFlushIdentity::reply_writer_timeout_attempt",
            "reply_writer_timeout_attempt",
            identity_context,
            ("#[must_use]",),
            "immutable admitted timeout-attempt projection",
        ),
        (
            "NetworkReplyFlushAck::poll",
            "poll",
            ack_context,
            (),
            "terminal reply-flush outcome classifier",
        ),
        (
            "ExactReplyWriterDeadline::expired_at",
            "expired_at",
            deadline_context,
            (),
            "monotone exact-reply deadline predicate",
        ),
        (
            "scaled_reply_writer_flush_timeout",
            "scaled_reply_writer_flush_timeout",
            (),
            (),
            "saturating adaptive reply-writer timeout scaler",
        ),
        (
            "AdmittedNetworkMessage::new_targeted_broadcast",
            "new_targeted_broadcast",
            admitted_context,
            (),
            "topology actor-item constructor",
        ),
        (
            "AdmittedNetworkMessage::new_targeted_post",
            "new_targeted_post",
            admitted_context,
            (),
            "exact-reply actor-item constructor",
        ),
        (
            "AdmittedNetworkMessage::into_dispatch_parts",
            "into_dispatch_parts",
            admitted_context,
            (),
            "reply-writer dispatch ownership split",
        ),
        (
            "AdmittedNetworkMessage::retain_after_dispatch_attempt",
            "retain_after_dispatch_attempt",
            admitted_context,
            (),
            "full-queue actor-item retention",
        ),
        (
            "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_at_attempt",
            "post_reply_recoverable_with_flush_ack_at_attempt",
            handle_context,
            ("#[allow(clippy::needless_pass_by_value)]",),
            "adaptive reply admission entry point",
        ),
        (
            "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_inner",
            "post_reply_recoverable_with_flush_ack_inner",
            handle_context,
            ("#[allow(clippy::needless_pass_by_value)]",),
            "exact reply completion minting",
        ),
        (
            "NetworkReplyFlushAckTestFixture::for_reply",
            "for_reply",
            flush_fixture_context,
            ("#[must_use]",),
            "default-attempt reply-flush test fixture",
        ),
        (
            "NetworkReplyFlushAckTestFixture::for_reply_at_attempt",
            "for_reply_at_attempt",
            flush_fixture_context,
            ("#[must_use]",),
            "attempt-aware reply-flush test fixture",
        ),
        (
            "exact_reply_flush_wins_terminal_fence",
            "exact_reply_flush_wins_terminal_fence",
            (),
            (),
            "exact reply close-and-immediate-poll terminal fence",
        ),
        (
            "AdmittedNetworkMessage::publish_ready_exact_reply_before_terminal_drop",
            "publish_ready_exact_reply_before_terminal_drop",
            admitted_context,
            (),
            "completion-bearing actor-item terminal-drop fence",
        ),
        (
            "ReliableActorPending::release_all_with_terminal_fence",
            "release_all_with_terminal_fence",
            pending_context,
            (),
            "pending-queue shutdown and abort fence",
        ),
        (
            "ReliableActorPending::release_cancelled_targets",
            "release_cancelled_targets",
            classified_pending_context,
            (),
            "pending-queue inactive-authority fence",
        ),
        (
            "NetworkBase::dispatch_reliable_actor_message",
            "dispatch_reliable_actor_message",
            actor_context,
            (),
            "zero-hook reliable dispatch wrapper",
        ),
        (
            "NetworkBase::dispatch_reliable_actor_message_inner",
            "dispatch_reliable_actor_message_inner",
            actor_context,
            (),
            "exact reply first-dispatch deadline and terminal-fence race kernel",
        ),
        (
            "NetworkBase::accept_reliable_actor_message",
            "accept_reliable_actor_message",
            actor_context,
            (),
            "inactive-authority admission-drop fence",
        ),
        (
            "NetworkBase::post_reliable_actor_frame_to_writer",
            "post_reliable_actor_frame_to_writer",
            actor_context,
            (),
            "bounded peer-writer admission kernel",
        ),
        (
            "NetworkBase::expire_reply_writer_occurrence",
            "expire_reply_writer_occurrence",
            actor_context,
            (),
            "exact accepting-connection timeout retirement",
        ),
    )
    context_qualified_items = {
        "NetworkReplyFlushAckTestFixture::for_reply",
        "NetworkReplyFlushAckTestFixture::for_reply_at_attempt",
    }
    for qualified_name, item_name, context, attributes, description in (
        item_contracts
    ):
        if qualified_name in context_qualified_items:
            matching = [
                candidate
                for candidate in rust_items(network_source, item_name)
                if candidate.brace_context == context
            ]
            if len(matching) != 1:
                errors.append(
                    f"{network_path}: require exactly one real Rust/Verus "
                    f"function item named {qualified_name}; found "
                    f"{len(matching)}"
                )
                item = None
            else:
                item = matching[0]
        else:
            item = _require_rust_item(
                network_path, network_source, item_name, errors
            )
        network_items[qualified_name] = item
        _require_rust_item_context(
            network_path,
            item,
            context,
            description,
            errors,
            expected_attributes=attributes,
        )
        _require_rust_item_token_sha256(
            network_path,
            item,
            _REPLY_WRITER_DEADLINE_NETWORK_ITEM_SHA256[qualified_name],
            description,
            errors,
        )

    dispatch_inner = network_items[
        "NetworkBase::dispatch_reliable_actor_message_inner"
    ]
    for constructor, description in (
        (
            """
let frame = RelayMessage::new_signed(
    &self.key_pair,
    RelayTarget::Direct(post.peer_id.clone()),
    self.relay_ttl,
    post.priority,
    post.data.clone(),
);
""",
            "exact direct reply dispatch must retain the authenticated relay origin",
        ),
        (
            """
let frame = RelayMessage::new_signed(
    &self.key_pair,
    RelayTarget::Broadcast,
    self.relay_ttl,
    broadcast.priority,
    broadcast.data.clone(),
);
""",
            "reliable broadcast dispatch must retain the authenticated relay origin",
        ),
    ):
        _require_rust_token_sequence(
            network_path,
            dispatch_inner,
            constructor,
            description,
            errors,
        )

    identity_constructor = network_items[
        "NetworkReplyFlushIdentity::from_admitted_ticket"
    ]
    _require_rust_token_sequence(
        network_path,
        identity_constructor,
        """
let ProgressDeliveryAuthority::Reply(route) = &ticket.authority else {
    return None;
};
let expected_authority = Some(ProgressAuthorityIdentity::Reply(
    route.tenure.connection_ordinal,
));
if ticket.shape.reply_writer_timeout_attempt.is_none()
    || ticket.shape.authority != expected_authority
    || ticket.source.target.as_ref() != Some(&route.tenure.delivery_peer)
    || ticket.shape.broadcast
{
    return None;
}
""",
        "release-mode identity construction must reject a missing timeout attempt or substituted reply shape",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "NetworkReplyFlushIdentity::reply_writer_timeout_attempt"
        ],
        """
self.ticket
    .shape
    .reply_writer_timeout_attempt
    .expect("reply flush identity construction requires a timeout attempt")
""",
        "the public timeout-attempt projection must be total after fail-closed construction",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items["NetworkReplyFlushAckTestFixture::for_reply"],
        "Self::for_reply_at_attempt(post, route, 0)",
        "the default test fixture must bind the base timeout attempt",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "NetworkReplyFlushAckTestFixture::for_reply_at_attempt"
        ],
        """
reply_writer_timeout_attempt: Some(reply_writer_timeout_attempt),
""",
        "the attempt-aware test fixture must retain the requested timeout generation",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_inner"
        ],
        """
self.submit_progress_message_to_source(
    message,
    topic,
    false,
    source,
    ProgressDeliveryAuthority::Reply(reply_route.clone()),
    ticket,
    Some(reply_writer_timeout_attempt),
    Some(reply_flush_sender),
)
""",
        "production admission must bind the caller's timeout generation into the exact ticket shape",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "NetworkBaseHandle::post_reply_recoverable_with_flush_ack_inner"
        ],
        """
let identity = NetworkReplyFlushIdentity::from_admitted_ticket(ticket)
    .expect("validated reply admission must retain its exact reply shape");
""",
        "validated admission must construct identity only from its exact retained reply shape",
        errors,
    )
    same_ticket = _require_rust_item(
        network_path, network_source, "same_ticket", errors
    )
    _require_rust_item_context(
        network_path,
        same_ticket,
        (("impl", "NetworkActorAdmittedTicketIdentity"),),
        "admitted-ticket equality including the timeout-attempt-bearing shape",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        same_ticket,
        """
Arc::ptr_eq(&self.budget, &other.budget)
    && self.id == other.id
    && self.rank == other.rank
    && self.shape == other.shape
    && self.source == other.source
""",
        "admitted ticket equality must include the complete attempt-bearing shape",
        errors,
    )
    try_reserve = _require_rust_item(
        network_path, network_source, "try_reserve_for_source", errors
    )
    _require_rust_item_context(
        network_path,
        try_reserve,
        (("impl", "NetworkActorProgressBudget"),),
        "release-mode retry-ticket shape validation",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        try_reserve,
        """
if !ticket.active
    || !Arc::ptr_eq(&ticket.budget, self)
    || ticket.shape != shape
    || ticket.source != source
""",
        "retry admission must reject a changed timeout-attempt-bearing shape before commit",
        errors,
    )

    pending_drop_context = (
        (
            "impl", "<", "T", ">", "Drop", "for",
            "ReliableActorPending", "<", "T", ">",
        ),
    )
    pending_drop_matches = [
        item
        for item in rust_items(network_source, "drop")
        if item.brace_context == pending_drop_context
    ]
    if len(pending_drop_matches) != 1:
        errors.append(
            f"{network_path}: require exactly one real ReliableActorPending "
            f"Drop item; found {len(pending_drop_matches)}"
        )
        pending_drop = None
    else:
        pending_drop = pending_drop_matches[0]
        _require_rust_item_context(
            network_path,
            pending_drop,
            pending_drop_context,
            "pending-queue abort terminal fence",
            errors,
        )
    _require_rust_token_sequence(
        network_path,
        pending_drop,
        "let _ = self.release_all_with_terminal_fence();",
        "pending-queue Drop must fence every retained exact occurrence",
        errors,
    )

    run_matches = [
        item
        for item in rust_items(network_source, "run")
        if item.brace_context == actor_context
    ]
    if len(run_matches) != 1:
        errors.append(
            f"{network_path}: require exactly one real network actor run item; "
            f"found {len(run_matches)}"
        )
        run = None
    else:
        run = run_matches[0]
        _require_rust_item_context(
            network_path,
            run,
            actor_context,
            "graceful network actor shutdown terminal fence",
            errors,
            expected_attributes=(
                "#[allow(clippy::too_many_lines)]",
                "#[log(skip(self, shutdown_signal), fields(listen_addr=%self.listen_addr, public_key=%self.key_pair.public_key()))]",
            ),
        )
    _require_rust_token_sequence(
        network_path,
        run,
        """
let released_on_shutdown = safety_dispatch_pending
    .release_all_with_terminal_fence()
    .saturating_add(progress_dispatch_pending.release_all_with_terminal_fence());
if released_on_shutdown > 0 {
    iroha_logger::debug!(
        released_on_shutdown,
        "Released reliable actor ownership through terminal fences at shutdown"
    );
}
let _ = self.cancel_all_reply_route_tenures();
""",
        "graceful shutdown must fence local exact receivers before route and writer teardown",
        errors,
    )

    dispatch_wrapper = network_items[
        "NetworkBase::dispatch_reliable_actor_message"
    ]
    _require_rust_token_sequence(
        network_path,
        dispatch_wrapper,
        "self.dispatch_reliable_actor_message_inner(admitted, || {})",
        "production dispatch must monomorphize the test seam to an empty hook",
        errors,
    )
    dispatch = network_items[
        "NetworkBase::dispatch_reliable_actor_message_inner"
    ]
    terminal_fence = network_items[
        "exact_reply_flush_wins_terminal_fence"
    ]
    _require_rust_token_sequence(
        network_path,
        terminal_fence,
        """
let Some(exact_target) = exact_target else {
    return false;
};
let Some(mut pending) = pending_flush_acks.remove(exact_target) else {
    return false;
};
pending.receiver.close();
matches!(pending.receiver.try_recv(), Ok(()))
""",
        "the terminal fence must remove one exact occurrence, close it, and poll immediately",
        errors,
    )
    terminal_drop = network_items[
        "AdmittedNetworkMessage::publish_ready_exact_reply_before_terminal_drop"
    ]
    _require_rust_token_sequence(
        network_path,
        terminal_drop,
        """
let exact_target = progress_authority.as_ref().and_then(|authority| {
    let ProgressDeliveryAuthority::Reply(route) = authority else {
        return None;
    };
    Some(route.semantic_target())
});
if !exact_reply_flush_wins_terminal_fence(pending_flush_acks, exact_target) {
    return false;
}
if let Some(reply_flush_ack) = reply_flush_ack.take() {
    let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed);
}
""",
        "terminal actor-item drop must fence only exact replies and publish only an observed flush",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "ReliableActorPending::release_cancelled_targets"
        ],
        """
entries.retain_mut(|entry| {
    if !entry.cancelled_progress_authority() {
        return true;
    }
    entry.publish_ready_exact_reply_before_terminal_drop();
    released = released.saturating_add(1);
    false
});
""",
        "inactive pending cleanup must fence each exact occurrence before removal",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items[
            "ReliableActorPending::release_all_with_terminal_fence"
        ],
        """
for entries in self.by_source.values_mut() {
    for entry in entries {
        entry.publish_ready_exact_reply_before_terminal_drop();
    }
}
self.by_source.clear();
self.ready_sources.clear();
self.ready_members.clear();
self.len = 0;
""",
        "shutdown cleanup must fence every pending exact occurrence before clearing ownership",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        network_items["NetworkBase::accept_reliable_actor_message"],
        """
if message.cancelled_progress_authority() {
    message.publish_ready_exact_reply_before_terminal_drop();
    return;
}
""",
        "early inactive-authority admission drop must use the terminal fence",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
reply_writer_deadline.get_or_insert_with(|| ExactReplyWriterDeadline {
    admitted_at: tokio::time::Instant::now(),
    timeout: scaled_reply_writer_flush_timeout(
        self.reply_writer_flush_timeout,
        attempt,
    ),
});
""",
        "the first actor dispatch must acquire one fixed adaptively scaled deadline before writer admission",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
let mut ack_targets: Vec<_> = pending_flush_acks.keys().cloned().collect();
ack_targets.sort();
""",
        "ready peer-writer receipts must be polled in deterministic order before route or timeout retirement",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        "after_initial_flush_poll();",
        "the deterministic test seam must run exactly after the optimistic flush poll",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
exact_reply_flush_wins_terminal_fence(
    &mut pending_flush_acks,
""",
        "inactive, replacement, and timeout exits must all use the one terminal fence",
        errors,
        count=3,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
let outcome = pending.receiver.try_recv();
match outcome {
    Ok(()) => {
        pending_flush_acks.remove(&target);
        completed_targets.insert(target);
    }
    Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
        pending_flush_acks.remove(&target);
        retry_targets.push(target);
    }
    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
}
""",
        "the writer receiver must linearize ready flush before route retirement and deadline expiry",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
if exact_reply_flushed {
    debug_assert!(reliable_progress);
    debug_assert!(matches!(&message, NetworkMessage::Post(_)));
    if let Some(reply_flush_ack) = reply_flush_ack {
        let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed);
    }
    drop(actor_lease);
    return Ok(());
}
""",
        "a ready exact receipt must publish Flushed before observing route retirement or replacement",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
if progress_authority
    .as_ref()
    .is_some_and(|authority| !authority.is_active())
{
    if exact_reply_flush_wins_terminal_fence(
        &mut pending_flush_acks,
        reply_route.as_ref().map(NetworkReplyRoute::semantic_target),
    ) {
        if let Some(reply_flush_ack) = reply_flush_ack {
            let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed);
        }
        drop(actor_lease);
        return Ok(());
    }
    drop(actor_lease);
    return Ok(());
}
""",
        "inactive-authority retirement must fence and publish a winning exact flush before dropping actor ownership",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
if !current_writer || !current_tenure {
    if exact_reply_flush_wins_terminal_fence(
        &mut pending_flush_acks,
        reply_route.as_ref().map(NetworkReplyRoute::semantic_target),
    ) {
        if let Some(reply_flush_ack) = reply_flush_ack {
            let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed);
        }
        drop(actor_lease);
        return Ok(());
    }
    route.tenure.mark_draining();
    let _ = self
        .network_actor_progress_budget
        .cancel_reply_route(&route.tenure);
""",
        "replacement retirement must fence before draining the old exact tenure",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
let timed_out_reply_writer = reply_route.is_some()
    && reply_writer_deadline
        .is_some_and(|deadline| deadline.expired_at(tokio::time::Instant::now()));
""",
        "only an exact reply may expire its actor-owned fixed deadline",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
if exact_reply_flush_wins_terminal_fence(&mut pending_flush_acks, Some(semantic_target))
{
    if let Some(reply_flush_ack) = reply_flush_ack {
        let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::Flushed);
    }
    drop(actor_lease);
    return Ok(());
}
let terminated_current_writer =
    self.expire_reply_writer_occurrence(&route, connection_id);
""",
        "deadline expiry must fence before retiring the exact writer occurrence",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
let terminated_current_writer =
    self.expire_reply_writer_occurrence(&route, connection_id);
""",
        "timeout must retire only the exact accepting connection occurrence",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
if let Some(reply_flush_ack) = reply_flush_ack {
    let _ = reply_flush_ack.send(NetworkReplyFlushCompletion::TimedOut);
}
drop(actor_lease);
return Ok(());
""",
        "deadline expiry must publish TimedOut rather than fabricate a flush",
        errors,
    )
    _require_rust_token_sequence(
        network_path,
        dispatch,
        """
reply_writer_timeout_attempt,
reply_writer_deadline,
reply_flush_ack,
""",
        "full writer-queue retry must retain adaptive attempt, absolute deadline, and completion sender unchanged",
        errors,
    )

    worker_items: dict[str, RustItem | None] = {}
    for qualified_name, item_name, context, description in (
        (
            "PendingExactOutput::handoff_applied_height_to_durable_reconstruction",
            "handoff_applied_height_to_durable_reconstruction",
            worker_context,
            "finality handoff timeout-attempt revalidation",
        ),
        (
            "PendingExactOutput::drive_with_budget_ack",
            "drive_with_budget_ack",
            worker_context,
            "timeout-attempt-bound exact reply admission",
        ),
        (
            "PendingExactOutput::poll_reply_flushes",
            "poll_reply_flushes",
            worker_context,
            "adaptive reply outcome application",
        ),
        (
            "PendingExactFanout::mark_admitted",
            "mark_admitted",
            fanout_context,
            "successful cursor admission and adaptive-attempt reset",
        ),
        (
            "PendingExactOutput::advance_after_attempt",
            "advance_after_attempt",
            worker_context,
            "successful cursor advance and attempt reset",
        ),
    ):
        item = _require_rust_item(worker_path, worker_source, item_name, errors)
        worker_items[qualified_name] = item
        _require_rust_item_context(
            worker_path,
            item,
            context,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            worker_path,
            item,
            _REPLY_WRITER_DEADLINE_WORKER_ITEM_SHA256[qualified_name],
            description,
            errors,
        )
    require(
        worker_path,
        """
struct PendingExactReplyFlush {
    flush_ack: NetworkReplyFlushAck,
    reply_writer_timeout_attempt: u8,
    sidecar_admission: Option<CertifiedMergeSidecarChunkAdmission>,
}
""",
        "each pending exact writer occurrence must retain its immutable admitted timeout attempt",
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::drive_with_budget_ack"],
        """
if !flush_ack
    .identity()
    .is_bound_to_canonical_reply(&canonical_post)
    || !flush_ack.identity().is_bound_to_delivery(&reply_route)
    || flush_ack.identity().reply_writer_timeout_attempt()
        != reply_writer_timeout_attempt
{
    return Err(
        "Sumeragi v2 ordinary reply flush changed route, payload, or timeout-attempt identity"
            .to_owned(),
    );
}
""",
        "ordinary reply installation must reject an acknowledgement from another timeout generation",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::drive_with_budget_ack"],
        """
if flush_ack.identity().reply_writer_timeout_attempt()
    != reply_writer_timeout_attempt
{
    return Err(
        "Sumeragi v2 sidecar reply flush changed timeout-attempt identity"
            .to_owned(),
    );
}
""",
        "sidecar reply installation must reject an acknowledgement from another timeout generation",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::drive_with_budget_ack"],
        """
PendingExactReplyFlush {
    flush_ack,
    reply_writer_timeout_attempt,
    sidecar_admission: None,
}
""",
        "ordinary reply installation must store the exact admitted timeout generation",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::drive_with_budget_ack"],
        """
PendingExactReplyFlush {
    flush_ack,
    reply_writer_timeout_attempt,
    sidecar_admission: Some(admission),
}
""",
        "sidecar reply installation must store the exact admitted timeout generation",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::poll_reply_flushes"],
        """
|| pending_flush.reply_writer_timeout_attempt != current_timeout_attempt
|| pending_flush
    .flush_ack
    .identity()
    .reply_writer_timeout_attempt()
    != pending_flush.reply_writer_timeout_attempt
""",
        "terminal reply-flush polling must preserve one adaptive-attempt identity across target, retained occurrence, and actor acknowledgement",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items[
            "PendingExactOutput::handoff_applied_height_to_durable_reconstruction"
        ],
        """
|| pending_flush.reply_writer_timeout_attempt
    != target.reply_writer_timeout_attempt
|| pending_flush
    .flush_ack
    .identity()
    .reply_writer_timeout_attempt()
    != pending_flush.reply_writer_timeout_attempt
""",
        "finality handoff must preserve target, retained occurrence, and acknowledgement timeout-attempt identity",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactOutput::poll_reply_flushes"],
        """
if matches!(status, NetworkReplyFlushAckStatus::TimedOut) {
    let target = self
        .fanouts
        .get_mut(fanout_index)
        .and_then(|fanout| fanout.targets.get_mut(target_index))
        .ok_or_else(|| {
            "Sumeragi v2 timed-out reply flush lost its target".to_owned()
        })?;
    target.reply_writer_timeout_attempt =
        target.reply_writer_timeout_attempt.saturating_add(1);
}
""",
        "only TimedOut may grow the adaptive attempt while Closed preserves it",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        worker_items["PendingExactFanout::mark_admitted"],
        "target.reply_writer_timeout_attempt = 0;",
        "only successful cursor advance resets the adaptive attempt",
        errors,
    )

    for test_name, expected_sha256 in (
        _REPLY_WRITER_DEADLINE_NETWORK_TEST_SHA256.items()
    ):
        test = _require_rust_item(
            network_path, network_source, test_name, errors
        )
        _require_rust_item_context(
            network_path,
            test,
            network_test_context,
            f"reply-writer deadline regression {test_name}",
            errors,
            expected_attributes=(
                ("#[test]",)
                if test_name
                in {
                    "reply_timeout_attempt_is_retained_by_actor_admission_ticket",
                    "reply_flush_identity_requires_and_exposes_timeout_attempt",
                    "reply_flush_test_fixture_distinguishes_success_timeout_and_close",
                    "adaptive_reply_timeout_scaling_handles_extreme_duration_without_panicking",
                    "terminal_fence_observes_send_before_close_and_rejects_send_after_close",
                }
                else ("#[tokio::test(start_paused = true)]",)
            ),
        )
        _require_rust_item_token_sha256(
            network_path,
            test,
            expected_sha256,
            f"reply-writer deadline regression {test_name}",
            errors,
        )
    for test_name, expected_sha256 in (
        _REPLY_WRITER_DEADLINE_WORKER_TEST_SHA256.items()
    ):
        test = _require_rust_item(
            worker_path, worker_source, test_name, errors
        )
        _require_rust_item_context(
            worker_path,
            test,
            worker_test_context,
            f"adaptive reply-attempt regression {test_name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            worker_path,
            test,
            expected_sha256,
            f"adaptive reply-attempt regression {test_name}",
            errors,
        )
    for test_name, expected_sha256 in (
        _REPLY_WRITER_DEADLINE_MERGE_TEST_SHA256.items()
    ):
        test = _require_rust_item(merge_path, merge_source, test_name, errors)
        _require_rust_item_context(
            merge_path,
            test,
            merge_test_context,
            f"sidecar adaptive reply-attempt regression {test_name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            merge_path,
            test,
            expected_sha256,
            f"sidecar adaptive reply-attempt regression {test_name}",
            errors,
        )
    return errors
