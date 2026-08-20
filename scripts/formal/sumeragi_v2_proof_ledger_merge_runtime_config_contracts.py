def _merge_runtime_config_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind config-format-v6 merge/pending limits to validation and live consumers."""

    # Mutation tests call this gate repeatedly with multi-megabyte Kura variants. Retain each
    # source's token stream only for the current complete cross-layer pass instead of allowing
    # the global token cache to pin every prior variant.
    rust_code_tokens.cache_clear()
    (
        kura_inventory_path,
        kura_inventory_source,
        kura_component_sources,
        kura_inventory_errors,
    ) = _kura_production_source_inventory(repo_root)
    paths = {
        "defaults": (
            repo_root
            / "crates"
            / "iroha_config"
            / "src"
            / "parameters"
            / "defaults.rs"
        ),
        "actual": (
            repo_root
            / "crates"
            / "iroha_config"
            / "src"
            / "parameters"
            / "actual.rs"
        ),
        "user": (
            repo_root
            / "crates"
            / "iroha_config"
            / "src"
            / "parameters"
            / "user.rs"
        ),
        "runner": (
            repo_root
            / "crates"
            / "iroha_core"
            / "src"
            / "sumeragi"
            / "v2_runner.rs"
        ),
        "lane": (
            repo_root
            / "crates"
            / "iroha_core"
            / "src"
            / "sumeragi"
            / "v2_lane_work.rs"
        ),
        "merge": (
            repo_root / "crates" / "iroha_core" / "src" / "merge_sidecar.rs"
        ),
        "kura": kura_inventory_path,
        "daemon": repo_root / "crates" / "irohad" / "src" / "main.rs",
    }
    errors: list[str] = []
    sources: dict[str, str] = {}
    for role, path in paths.items():
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: merge-runtime {role} source must be a regular file"
            )
            sources[role] = ""
        else:
            sources[role] = path.read_text(encoding="utf-8")
    errors.extend(kura_inventory_errors)
    sources["kura"] = kura_inventory_source

    if len(MERGE_RUNTIME_CONFIG_FIELDS) != 16:
        errors.append(
            "merge-runtime shared-config inventory must contain exactly 16 fields"
        )

    defaults_path = paths["defaults"]
    defaults_source = sources["defaults"]
    actual_path = paths["actual"]
    actual_source = sources["actual"]
    user_path = paths["user"]
    user_source = sources["user"]
    runner_path = paths["runner"]
    runner_source = sources["runner"]
    lane_path = paths["lane"]
    lane_source = sources["lane"]
    merge_path = paths["merge"]
    merge_source = sources["merge"]
    kura_path = paths["kura"]
    kura_source = sources["kura"]
    daemon_path = paths["daemon"]
    daemon_source = sources["daemon"]

    # Completed semantic requests are retired only by authenticated cumulative
    # close floors. Reintroducing a wall-clock tombstone TTL at any config or
    # production seam would let elapsed time reopen an exact request.
    for role, path in paths.items():
        errors.extend(
            _retired_sidecar_gate_ttl_source_errors(
                path,
                sources[role],
                role,
            )
        )
    for relative, path, source in kura_component_sources:
        errors.extend(
            _retired_sidecar_gate_ttl_source_errors(
                path,
                source,
                f"Kura production component {relative}",
            )
        )

    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        "pub const SUMERAGI_V2_CONFIG_FORMAT_VERSION: u16 = 6;",
        "merge-runtime shared-config format version 6",
        errors,
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        """
#[derive(Clone, Debug, PartialEq, Eq, Encode)]
pub struct SumeragiV2Config {
    pub format_version: u16,
    pub protocol_version: u16,
    pub mode: consensus_v2::ConsensusMode,
    pub block_cadence_ms: u64,
    pub limits: SumeragiV2Limits,
    pub key_policy: SumeragiV2KeyPolicy,
}
""",
        "canonical shared config encodes the complete limits projection",
        errors,
    )
    _require_rust_source_token_sequence(
        defaults_path,
        defaults_source,
        """
pub const V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES: usize = 64 * 1024;
""",
        "merge-signing metadata headroom has one named config source",
        errors,
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        """
let encoded = self.encode();
let mut preimage =
    Vec::with_capacity(SUMERAGI_V2_CONFIG_FINGERPRINT_DOMAIN.len() + encoded.len());
preimage.extend_from_slice(SUMERAGI_V2_CONFIG_FINGERPRINT_DOMAIN);
preimage.extend_from_slice(&encoded);
Hash::new(preimage)
""",
        "handshake fingerprint hashes the complete encoded config-v6 projection",
        errors,
    )

    actual_fields = "\n".join(
        f"pub {actual_field}: {actual_type},"
        for (
            _projected_field,
            actual_field,
            _user_field,
            _default_constant,
            actual_type,
            _user_type,
            _user_default_suffix,
            _user_mapping_suffix,
        ) in MERGE_RUNTIME_CONFIG_FIELDS
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        actual_fields,
        "actual runtime limits carry all 16 config-v6 merge fields in order",
        errors,
    )

    shared_fields = "\n".join(
        f"pub {projected_field}: u64,"
        for projected_field, *_rest in MERGE_RUNTIME_CONFIG_FIELDS
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        shared_fields,
        "shared fingerprint limits carry all 16 config-v6 merge fields in order",
        errors,
    )

    actual_defaults = "\n".join(
        f"{actual_field}: defaults::sumeragi::{default_constant},"
        for (
            _projected_field,
            actual_field,
            _user_field,
            default_constant,
            _actual_type,
            _user_type,
            _user_default_suffix,
            _user_mapping_suffix,
        ) in MERGE_RUNTIME_CONFIG_FIELDS
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        actual_defaults,
        "actual runtime defaults source all 16 config-v6 merge fields",
        errors,
    )

    user_mapping = "\n".join(
        f"{actual_field}: limits.{user_field}{user_mapping_suffix},"
        for (
            _projected_field,
            actual_field,
            user_field,
            _default_constant,
            _actual_type,
            _user_type,
            _user_default_suffix,
            user_mapping_suffix,
        ) in MERGE_RUNTIME_CONFIG_FIELDS
    )
    _require_rust_source_token_sequence(
        user_path,
        user_source,
        user_mapping,
        "user parsing maps all 16 config-v6 merge fields without substitution",
        errors,
    )

    projected_fields = "\n".join(
        f"{projected_field},"
        for projected_field, *_rest in MERGE_RUNTIME_CONFIG_FIELDS
    )
    _require_rust_source_token_sequence(
        actual_path,
        actual_source,
        projected_fields,
        "shared fingerprint projection carries all 16 config-v6 merge fields",
        errors,
    )

    for (
        _projected_field,
        _actual_field,
        user_field,
        default_constant,
        _actual_type,
        user_type,
        user_default_suffix,
        _user_mapping_suffix,
    ) in MERGE_RUNTIME_CONFIG_FIELDS:
        default_declarations = re.findall(
            rf"(?m)^\s*pub const {re.escape(default_constant)}\s*:",
            defaults_source,
        )
        if len(default_declarations) != 1:
            errors.append(
                f"{defaults_path}: config-v6 default {default_constant} must be "
                f"declared exactly once; found {len(default_declarations)}"
            )
        default_expression = (
            f"defaults::sumeragi::{default_constant}{user_default_suffix}"
        )
        user_declarations = re.findall(
            rf'#\[config\(\s*default\s*=\s*"{re.escape(default_expression)}"\s*\)\]'
            rf"\s*pub\s+{re.escape(user_field)}\s*:\s*{re.escape(user_type)}\s*,",
            user_source,
        )
        if len(user_declarations) != 1:
            errors.append(
                f"{user_path}: user config field {user_field} must bind default "
                f"{default_expression} exactly once; found {len(user_declarations)}"
            )

    for expected, description in (
        (
            """
let merge_sidecar_inbound_session_capacity = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_inbound_session_capacity",
    self.limits.merge_sidecar_inbound_session_capacity.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_inbound_session_capacity",
    merge_sidecar_inbound_session_capacity,
    2,
)?;
let merge_sidecar_inbound_sessions_per_peer = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_inbound_sessions_per_peer",
    self.limits.merge_sidecar_inbound_sessions_per_peer.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_inbound_sessions_per_peer",
    merge_sidecar_inbound_sessions_per_peer,
    2,
)?;
require_maximum(
    "sumeragi.limits.merge_sidecar_inbound_sessions_per_peer",
    merge_sidecar_inbound_sessions_per_peer,
    merge_sidecar_inbound_session_capacity,
)?;
""",
            "config validation preserves decided and ordinary inbound session corridors",
        ),
        (
            """
let merge_sidecar_inbound_assembly_bytes = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_inbound_assembly_bytes",
    self.limits.merge_sidecar_inbound_assembly_bytes.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_inbound_assembly_bytes",
    merge_sidecar_inbound_assembly_bytes,
    canonical_size(
        "Sumeragi v2 merge-sidecar inbound byte minimum",
        defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_MIN,
    )?,
)?;
let merge_sidecar_inbound_assembly_bytes_per_peer = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_inbound_assembly_bytes_per_peer",
    self.limits
        .merge_sidecar_inbound_assembly_bytes_per_peer
        .get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_PER_PEER_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_inbound_assembly_bytes_per_peer",
    merge_sidecar_inbound_assembly_bytes_per_peer,
    canonical_size(
        "Sumeragi v2 per-peer merge-sidecar inbound byte minimum",
        defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_MIN,
    )?,
)?;
require_maximum(
    "sumeragi.limits.merge_sidecar_inbound_assembly_bytes_per_peer",
    merge_sidecar_inbound_assembly_bytes_per_peer,
    merge_sidecar_inbound_assembly_bytes,
)?;
""",
            "config validation preserves global and per-peer inbound byte corridors",
        ),
        (
            """
let merge_sidecar_deferred_block_capacity = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_deferred_block_capacity",
    self.limits.merge_sidecar_deferred_block_capacity.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_DEFERRED_BLOCK_CAPACITY_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_deferred_block_capacity",
    merge_sidecar_deferred_block_capacity,
    2,
)?;
let merge_sidecar_future_block_distance = canonical_bounded_u64(
    "sumeragi.limits.merge_sidecar_future_block_distance",
    self.limits.merge_sidecar_future_block_distance.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_FUTURE_BLOCK_DISTANCE_MAX,
)?;
let merge_sidecar_request_timeout_ms = canonical_duration_ms(
    "sumeragi.limits.merge_sidecar_request_timeout_ms",
    self.limits.merge_sidecar_request_timeout,
)?;
require_maximum(
    "sumeragi.limits.merge_sidecar_request_timeout_ms",
    merge_sidecar_request_timeout_ms,
    defaults::sumeragi::V2_MERGE_SIDECAR_REQUEST_TIMEOUT_MAX_MS,
)?;
""",
            "config validation bounds deferred work, future distance, and retry time",
        ),
        (
            """
let merge_sidecar_outbound_sessions_per_source = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_outbound_sessions_per_source",
    self.limits.merge_sidecar_outbound_sessions_per_source.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE_MAX,
)?;
let merge_sidecar_outbound_bytes_per_source = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_outbound_bytes_per_source",
    self.limits.merge_sidecar_outbound_bytes_per_source.get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_outbound_bytes_per_source",
    merge_sidecar_outbound_bytes_per_source,
    canonical_size(
        "Sumeragi v2 merge-sidecar outbound byte minimum",
        defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE_MIN,
    )?,
)?;
let merge_sidecar_server_request_gates_per_source = canonical_bounded_size(
    "sumeragi.limits.merge_sidecar_server_request_gates_per_source",
    self.limits
        .merge_sidecar_server_request_gates_per_source
        .get(),
    defaults::sumeragi::V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_sidecar_server_request_gates_per_source",
    merge_sidecar_server_request_gates_per_source,
    merge_sidecar_outbound_sessions_per_source,
)?;
""",
            "config validation binds per-source output and gate geometry",
        ),
        (
            """
let pending_certified_merge_entry_capacity = canonical_bounded_size(
    "sumeragi.limits.pending_certified_merge_entry_capacity",
    self.limits.pending_certified_merge_entry_capacity.get(),
    defaults::sumeragi::V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY_MAX,
)?;
let pending_queue_plan_admission_capacity = canonical_bounded_size(
    "sumeragi.limits.pending_queue_plan_admission_capacity",
    self.limits.pending_queue_plan_admission_capacity.get(),
    defaults::sumeragi::V2_PENDING_QUEUE_PLAN_ADMISSION_CAPACITY_MAX,
)?;
let pending_control_sidecar_bytes = canonical_bounded_size(
    "sumeragi.limits.pending_control_sidecar_bytes",
    self.limits.pending_control_sidecar_bytes.get(),
    defaults::sumeragi::V2_PENDING_CONTROL_SIDECAR_BYTES_MAX,
)?;
require_minimum(
    "sumeragi.limits.pending_control_sidecar_bytes",
    pending_control_sidecar_bytes,
    u64::try_from(defaults::sumeragi::V2_PENDING_CONTROL_SIDECAR_BYTES_MIN)
        .expect("static pending-control sidecar byte minimum fits u64"),
)?;
""",
            "config validation bounds pending merge, QueuePlan, and shared bytes",
        ),
        (
            """
let merge_signing_guard_record_capacity = canonical_bounded_size(
    "sumeragi.limits.merge_signing_guard_record_capacity",
    self.limits.merge_signing_guard_record_capacity.get(),
    defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_CAPACITY_MAX,
)?;
let merge_signing_guard_record_bytes = canonical_bounded_size(
    "sumeragi.limits.merge_signing_guard_record_bytes",
    self.limits.merge_signing_guard_record_bytes.get(),
    defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_BYTES_MAX,
)?;
require_minimum(
    "sumeragi.limits.merge_signing_guard_record_bytes",
    merge_signing_guard_record_bytes,
    canonical_size(
        "Sumeragi v2 merge-signing record byte minimum",
        defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_BYTES_MIN,
    )?,
)?;
let merge_signing_guard_total_bytes = canonical_bounded_size(
    "sumeragi.limits.merge_signing_guard_total_bytes",
    self.limits.merge_signing_guard_total_bytes.get(),
    defaults::sumeragi::V2_MERGE_SIGNING_GUARD_TOTAL_BYTES_MAX,
)?;
let merge_signing_guard_minimum_total_bytes = merge_signing_guard_record_bytes
    .checked_add(
        u64::try_from(defaults::sumeragi::V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES)
            .expect("static merge-signing metadata headroom fits u64"),
    )
    .ok_or(SumeragiV2ConfigError::LimitOverflow(
        "Sumeragi v2 merge-signing aggregate byte minimum",
    ))?;
require_minimum(
    "sumeragi.limits.merge_signing_guard_total_bytes",
    merge_signing_guard_total_bytes,
    merge_signing_guard_minimum_total_bytes.max(
        u64::try_from(defaults::sumeragi::V2_MERGE_SIGNING_GUARD_TOTAL_BYTES_MIN)
            .expect("static merge-signing minimum fits u64"),
    ),
)?;
""",
            "config validation bounds merge-signing count, record, and aggregate bytes",
        ),
    ):
        _require_rust_source_token_sequence(
            actual_path,
            actual_source,
            expected,
            description,
            errors,
        )

    _require_rust_source_token_sequence(
        runner_path,
        runner_source,
        """
let merge_sidecar_limits = MergeSidecarLimits::new(
    non_zero(config.limits.merge_sidecar_inbound_session_capacity)?,
    non_zero(config.limits.merge_sidecar_inbound_sessions_per_peer)?,
    non_zero(config.limits.merge_sidecar_inbound_assembly_bytes)?,
    non_zero(config.limits.merge_sidecar_inbound_assembly_bytes_per_peer)?,
    non_zero(config.limits.merge_sidecar_deferred_block_capacity)?,
    NonZeroU64::new(config.limits.merge_sidecar_future_block_distance)
        .ok_or(V2RunnerError::InvalidLimits)?,
    Duration::from_millis(merge_sidecar_request_timeout_ms.get()),
    non_zero(config.limits.merge_sidecar_outbound_sessions_per_source)?,
    non_zero(config.limits.merge_sidecar_outbound_bytes_per_source)?,
    non_zero(config.limits.merge_sidecar_server_request_gates_per_source)?,
)
.map_err(|_| V2RunnerError::InvalidLimits)?;
let merge_signing_guard_limits = MergeSigningGuardLimits::new(
    non_zero(config.limits.merge_signing_guard_record_capacity)?,
    non_zero(config.limits.merge_signing_guard_record_bytes)?,
    non_zero(config.limits.merge_signing_guard_total_bytes)?,
)
.map_err(|_| V2RunnerError::InvalidLimits)?;
""",
        "runner constructs live sidecar and signing limits from all projected merge fields",
        errors,
    )
    _require_rust_source_token_sequence(
        runner_path,
        runner_source,
        """
non_zero(config.limits.sidecar_service_burst)?,
merge_sidecar_limits,
merge_signing_guard_limits,
native_amx_signing_guard_limits,
""",
        "runner transfers validated merge limits into the height-local adapter",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        """
let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(
    &kura.store_root(),
    committed_merge_epoch,
    state_height,
    limits.merge_signing_guard_limits,
)
""",
        "adapter opens the durable merge-signing journal with fingerprinted limits",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        """
const fn merge_sidecar_server_stream_capacity(roster_len: usize) -> usize {
    roster_len + wire::MAX_VALIDATORS_PER_HEIGHT
}
""",
        "adapter sidecar server stream capacity reserves current and predecessor committees",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        """
let sidecar_server_roster = context
    .roster
    .iter()
    .map(|entry| entry.validator.clone())
    .collect::<Vec<_>>();
let sidecar_server_stream_capacity =
    merge_sidecar_server_stream_capacity(sidecar_server_roster.len());
let sidecar_server_roster_digest =
    canonical_merge_sidecar_roster_digest(&sidecar_server_roster);
let merge_sidecars = match retained_merge_sidecars {
    Some(retained) => retained.rehydrate_for_successor(
        &context,
        limits.reply_source_capacity.get(),
        limits.merge_sidecar_limits,
        sidecar_server_stream_capacity,
        sidecar_server_roster_digest,
        Instant::now(),
    ),
    None => MergeSidecarTransport::open_durable_with_server_stream_capacity(
        &kura.store_root(),
        limits.reply_source_capacity.get(),
        limits.merge_sidecar_limits,
        sidecar_server_stream_capacity,
        sidecar_server_roster_digest,
    )
    .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string())),
}
?;
""",
        "adapter must derive the canonical responder roster and restore or open only its "
        "exact durable source, stream, and roster geometry",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        """
merge_signing_guard,
merge_sidecars,
predecessor_sidecar_requesters: None,
exact_output_handoff_owner,
authenticated_merge_qcs: BTreeSet::new(),
""",
        "adapter hands the exact rehydrated sidecar transport into the live production field",
        errors,
    )
    _require_rust_source_token_sequence(
        merge_path,
        merge_source,
        """
let metadata_headroom =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES;
let minimum_record_bytes = MAX_MERGE_LEDGER_ENTRY_BYTES
    .checked_add(metadata_headroom)
""",
        "live merge-signing geometry consumes the named metadata headroom",
        errors,
    )
    kura_constructor = (
        "new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits"
    )
    kura_structural_source = mask_rust_comments_and_literals(kura_source)
    kura_items: dict[str, RustItem | None] = {}
    for name in (
        kura_constructor,
        "pending_merge_entry_paths_unlocked",
        "pending_queue_plan_admission_paths_unlocked",
        "validate_pending_merge_entries_on_startup",
        "persist_pending_certified_merge_entry",
        "persist_pending_queue_plan_admission_certificate",
    ):
        items = rust_function_items_from_structural(
            kura_source, kura_structural_source, name
        )
        if len(items) != 1:
            errors.append(
                f"{kura_path}: require exactly one real Rust function item named "
                f"{name}; found {len(items)}"
            )
            kura_items[name] = None
        else:
            kura_items[name] = items[0]
    _require_rust_source_token_sequence(
        daemon_path,
        daemon_source,
        """
Kura::new_with_configured_lane_catalog_and_snapshot_bootstrap_and_sumeragi_limits(
    &config.kura,
    &config.nexus.lane_config,
    &config.nexus.configured_lane_catalog,
    &config.snapshot.bootstrap,
    &config.sumeragi.limits,
)
""",
        "daemon passes fingerprinted pending-control limits into production Kura",
        errors,
    )
    _require_rust_token_sequence(
        kura_path,
        kura_items[kura_constructor],
        """
let pending_control_sidecar_limits = PendingControlSidecarLimits::from_config(
    sumeragi_limits,
    &config.store_dir.resolve_relative_path(),
)?;
""",
        "Kura validates pending-control limits before opening its store",
        errors,
    )
    for item_name, expected, description in (
        (
            "pending_merge_entry_paths_unlocked",
            """
if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
    return Err(Self::invalid_pending_merge_entry_error(
        directory,
        "pending certified merge entry count exceeds the hard limit",
    ));
}
""",
            "Kura restart inventory consumes the configured pending merge count",
        ),
        (
            "persist_pending_certified_merge_entry",
            """
if paths.len() == self.pending_control_sidecar_limits.certified_merge_entries {
    return Err(Self::invalid_pending_merge_entry_error(
        directory,
        "pending certified merge entry count exceeds the hard limit",
    ));
}
""",
            "Kura merge admission consumes the configured pending-entry count",
        ),
        (
            "pending_queue_plan_admission_paths_unlocked",
            """
if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
    return Err(Self::invalid_pending_queue_plan_admission_error(
        directory,
        "pending QueuePlan admission certificate count exceeds the hard limit",
    ));
}
""",
            "Kura restart inventory consumes the configured pending QueuePlan count",
        ),
        (
            "persist_pending_queue_plan_admission_certificate",
            """
if paths.len() == self.pending_control_sidecar_limits.queue_plan_admissions {
    return Err(Self::invalid_pending_queue_plan_admission_error(
        directory,
        "pending QueuePlan admission certificate count exceeds the hard limit",
    ));
}
""",
            "Kura QueuePlan admission consumes the configured certificate count",
        ),
        (
            "validate_pending_merge_entries_on_startup",
            """
if !self
    .pending_control_sidecar_limits
    .combined_bytes_within_limit(merge_bytes, admission_bytes)
{
    return Err(Self::invalid_pending_queue_plan_admission_error(
        self.store_root.clone(),
        "pending merge and QueuePlan admission sidecars exceed their shared hard byte limit",
    ));
}
""",
            "Kura startup consumes the configured shared pending byte limit",
        ),
        (
            "persist_pending_certified_merge_entry",
            """
if pending_bytes.checked_add(bytes.len()).is_none_or(|total| {
    !self
        .pending_control_sidecar_limits
        .combined_bytes_within_limit(total, admission_bytes)
}) {
""",
            "Kura merge admission consumes the configured shared pending byte limit",
        ),
        (
            "persist_pending_queue_plan_admission_certificate",
            """
if admission_bytes
    .checked_add(canonical_certificate_bytes.len())
    .is_none_or(|total| {
        !self
            .pending_control_sidecar_limits
            .combined_bytes_within_limit(merge_bytes, total)
    })
{
""",
            "Kura QueuePlan admission consumes the configured shared pending byte limit",
        ),
    ):
        _require_rust_token_sequence(
            kura_path,
            kura_items[item_name],
            expected,
            description,
            errors,
        )

    for expected, description in (
        (
            """
if inbound_session_capacity <= RESERVED_DECIDED_INBOUND_SESSIONS
    || inbound_sessions_per_peer <= RESERVED_DECIDED_INBOUND_SESSIONS
    || inbound_sessions_per_peer > inbound_session_capacity
    || deferred_block_capacity <= RESERVED_DECIDED_DEFERRED_BLOCKS
    || inbound_assembly_bytes < minimum_inbound_bytes
    || inbound_assembly_bytes_per_peer < minimum_inbound_bytes
    || inbound_assembly_bytes_per_peer > inbound_assembly_bytes
    || outbound_bytes_per_source < MAX_MERGE_LEDGER_ENTRY_BYTES
    || server_request_gates_per_source < outbound_sessions_per_source
    || request_timeout.is_zero()
""",
            "live sidecar constructor revalidates every relational corridor",
        ),
        (
            """
let outbound_session_capacity = reply_source_capacity
    .checked_mul(limits.outbound_sessions_per_source)
    .ok_or(MergeSidecarError::Capacity(
        "outbound response session geometry",
    ))?;
let outbound_byte_capacity = reply_source_capacity
    .checked_mul(limits.outbound_bytes_per_source)
    .ok_or(MergeSidecarError::Capacity(
        "outbound response byte geometry",
    ))?;
let (server_request_gate_capacity, server_request_attempt_capacity) =
    Self::derive_server_request_capacities(
        reply_source_capacity,
        limits,
        server_stream_capacity,
    )?;
""",
            "live sidecar transport derives checked source-partition capacities",
        ),
        (
            """
height > committed_height.saturating_add(self.limits.future_block_distance)
""",
            "live sidecar carrier admission consumes configured future distance",
        ),
        (
            """
self.deferred_count() >= self.limits.deferred_block_capacity
""",
            "live sidecar admission consumes configured deferred-block capacity",
        ),
        (
            """
self.inbound.len() >= self.limits.inbound_session_capacity
""",
            "live sidecar admission consumes configured global session capacity",
        ),
        (
            """
new_global_bytes > self.limits.inbound_assembly_bytes
""",
            "live sidecar ingestion consumes configured global byte capacity",
        ),
        (
            """
new_peer_bytes > self.limits.inbound_assembly_bytes_per_peer
""",
            "live sidecar ingestion consumes configured per-peer byte capacity",
        ),
        (
            """
self.inbound_peer_session_count(holder)
    < self.limits.inbound_sessions_per_peer
""",
            "live sidecar scheduling consumes configured per-peer session capacity",
        ),
        (
            """
now.saturating_duration_since(attempt.last_progress_at)
    >= retry_timeout(self.limits.request_timeout, assembly.attempts)
""",
            "live sidecar retry consumes configured request timeout",
        ),
        (
            """
self.source_outbound_count(source) < self.limits.outbound_sessions_per_source
""",
            "live sidecar response admission consumes configured source sessions",
        ),
        (
            """
self.source_outbound_bytes(source).saturating_add(bytes)
    <= self.limits.outbound_bytes_per_source
""",
            "live sidecar response admission consumes configured source bytes",
        ),
        (
            """
self.source_gate_count(&source) >= self.limits.server_request_gates_per_source
""",
            "live sidecar request admission consumes configured source gates",
        ),
        (
            """
Self::guard_directory_bytes(&directory, limits.max_total_bytes)?;
Self::reconcile_temps(&directory, limits)?;
let durable_high_water = Self::read_high_water(&directory, limits.max_record_bytes)?
""",
            "merge-signing startup consumes configured record and aggregate byte limits",
        ),
        (
            """
if bytes.len() > self.limits.max_record_bytes
""",
            "merge-signing authorization consumes configured record bytes",
        ),
        (
            """
if count >= self.limits.max_records {
    return Err(MergeSidecarError::SigningGuard(
        "signing-guard record count reached hard limit".to_owned(),
    ));
}
""",
            "merge-signing authorization consumes configured record count",
        ),
        (
            """
if count >= self.limits.max_records {
    return Err(MergeSidecarError::SigningGuard(
        "signing-guard record count reached hard limit".to_owned(),
    ));
}
if total_bytes
    .checked_add(bytes.len())
    .is_none_or(|total| total > self.limits.max_total_bytes)
""",
            "merge-signing authorization consumes configured aggregate bytes",
        ),
    ):
        _require_rust_source_token_sequence(
            merge_path,
            merge_source,
            expected,
            description,
            errors,
        )

    return errors

def _lifecycle_capacity_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind admitted network geometry to the height-local lifecycle ledger."""

    relative_paths = {
        "actual": "crates/iroha_config/src/parameters/actual.rs",
        "user": "crates/iroha_config/src/parameters/user.rs",
        "defaults": "crates/iroha_config/src/parameters/defaults.rs",
        "authority": "crates/iroha_core/src/sumeragi/v2_lifecycle_authority.rs",
        "schema": "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
        "worker": "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "consensus": "crates/iroha_data_model/src/block/consensus_v2.rs",
        "test_network": "crates/iroha_test_network/src/lib.rs",
        "izanami": "crates/izanami/src/chaos.rs",
        "kagami": "crates/iroha_kagami/src/localnet.rs",
    }
    errors: list[str] = []
    paths: dict[str, Path] = {}
    sources: dict[str, str] = {}
    for role, relative in relative_paths.items():
        path, source = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            f"lifecycle-capacity {role} source",
        )
        paths[role] = path
        sources[role] = source

    helper = _require_rust_item(
        paths["actual"],
        sources["actual"],
        "validate_sumeragi_v2_lifecycle_capacity_geometry",
        errors,
    )
    _require_rust_item_context(
        paths["actual"],
        helper,
        (),
        "lifecycle-capacity config kernel",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["actual"],
        helper,
        "7315b7e8e644464ed3c7e429b9354312c3afc59411e97a6fa1a1987c89a78eac",
        "lifecycle-capacity config kernel",
        errors,
    )
    _require_rust_token_sequence(
        paths["actual"],
        helper,
        """
let consensus = defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP
    .checked_mul(2)
    .ok_or(SumeragiV2LifecycleCapacityGeometryError::ArithmeticOverflow)?;
let observer_owners = reply_source_capacity
    .max(1)
    .checked_mul(certified_request_capacity)
    .ok_or(SumeragiV2LifecycleCapacityGeometryError::ArithmeticOverflow)?;
let serve = consensus_v2::MAX_VALIDATORS_PER_HEIGHT
    .checked_add(observer_owners)
    .and_then(|owners| owners.checked_mul(2))
    .ok_or(SumeragiV2LifecycleCapacityGeometryError::ArithmeticOverflow)?;
let actual = serve
    .checked_mul(2)
""",
        "lifecycle capacity must use checked consensus, observer, two-phase Serve, and Producer geometry",
        errors,
    )
    _require_rust_token_sequence(
        paths["actual"],
        helper,
        """
let maximum = usize::from(u16::MAX) + 1;
if actual > maximum {
    return Err(SumeragiV2LifecycleCapacityGeometryError::CapacityTooLarge {
        actual,
        maximum
    });
}
Ok(actual)
""",
        "lifecycle capacity must reject every inventory beyond the u16 slot namespace",
        errors,
    )

    root_parse = _require_qualified_rust_item(
        paths["user"],
        sources["user"],
        "Root",
        "parse",
        errors,
        "lifecycle-capacity root configuration admission",
        expected_attributes=("#[allow(clippy::too_many_lines)]",),
    )
    _require_rust_token_sequence(
        paths["user"],
        root_parse,
        """
if let Err(error) = actual::validate_sumeragi_v2_lifecycle_capacity_geometry(
    effect_work_capacity,
    sumeragi.queues.bodies.get(),
    reply_source_capacity,
) {
    emitter.emit(
        Report::new(ParseError::InvalidSumeragiConfig).attach(format!(
            "{error}; configured network reply-source capacity is {reply_source_capacity}"
        )),
    );
}
""",
        "root parsing must reject lifecycle geometry before constructing the runtime config",
        errors,
    )

    runtime_geometry = _require_rust_item(
        paths["authority"],
        sources["authority"],
        "capacity_geometry_from_limits",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["authority"],
        runtime_geometry,
        "d16a738354e503ec42f20195a898359f3a868ddbcacad72b224c6d4ad5e6d2be",
        "runtime lifecycle-capacity authority",
        errors,
    )
    _require_rust_token_sequence(
        paths["authority"],
        runtime_geometry,
        """
validate_sumeragi_v2_lifecycle_capacity_geometry(
    effect_work_capacity,
    certified_request_capacity,
    reply_route_source_capacity,
)
.ok()?;
let consensus = MAX_EFFECTS_PER_STEP.checked_mul(2)?;
let serve = certified_serve_family_capacity(
    roster_len,
    reply_route_source_capacity.max(1),
    certified_request_capacity,
)
.ok()?;
""",
        "runtime lifecycle authority must consume the shared checked geometry before minting slots",
        errors,
    )
    _require_rust_token_sequence(
        paths["authority"],
        runtime_geometry,
        """
let geometry = CapacityGeometry::new([
    (CapacityClass::Consensus, consensus),
    (CapacityClass::Effect, effect_work_capacity),
    (CapacityClass::Serve, serve),
    (CapacityClass::Producer, serve),
]);
""",
        "runtime lifecycle authority must retain the same four capacity classes",
        errors,
    )

    for role, expected, description in (
        (
            "defaults",
            """
pub const CORE_MAX_TOTAL_CONNECTIONS: usize = 97;
""",
            "the shared Core connection fallback must retain the reviewed 97-source boundary",
        ),
        (
            "schema",
            """
pub(super) const MAX_LIFECYCLE_RECORDS_PER_HEIGHT: usize = u16::MAX as usize + 1;
""",
            "the lifecycle ledger must retain the 65,536-record physical namespace",
        ),
        (
            "worker",
            """
const CERTIFIED_SERVE_PHASE_FAMILIES: usize = 2;
""",
            "Serve ownership must retain both phase families",
        ),
        (
            "consensus",
            """
pub const MAX_VALIDATORS_PER_HEIGHT: usize = 3 * MAX_FAULTS_PER_HEIGHT + 1;
""",
            "lifecycle admission must remain tied to the protocol validator ceiling",
        ),
        (
            "kagami",
            """
const LOCALNET_SUMERAGI_QUEUE_COMMANDS: usize = 8_192;
""",
            "Kagami must retain the reviewed high-command localnet profile",
        ),
        (
            "kagami",
            """
const LOCALNET_SUMERAGI_QUEUE_BODIES: usize =
    iroha_config::parameters::defaults::sumeragi::QUEUE_BODY_CAPACITY.get();
""",
            "Kagami must inherit the canonical 163-body capacity",
        ),
        (
            "izanami",
            """
const IZANAMI_MAX_TOTAL_CONNECTIONS: i64 = 31;
""",
            "Izanami's 512-body profile must retain its reviewed 31-source cap",
        ),
        (
            "izanami",
            """
const IZANAMI_SUMERAGI_QUEUE_COMMANDS: i64 = 4_096;
const IZANAMI_SUMERAGI_QUEUE_BODIES: i64 = 512;
""",
            "Izanami must retain the high-load command/body geometry guarded by that cap",
        ),
        (
            "izanami",
            """
.write(
    ["network", "max_total_connections"],
    IZANAMI_MAX_TOTAL_CONNECTIONS,
)
""",
            "Izanami must publish its lifecycle-safe source cap into the generated config",
        ),
    ):
        _require_rust_source_token_sequence(
            paths[role],
            sources[role],
            expected,
            description,
            errors,
        )

    _require_rust_source_token_sequence(
        paths["test_network"],
        sources["test_network"],
        """
.write(["sumeragi", "queues", "bodies"], 512i64)
""",
        "test networks must not restore the lifecycle-invalid 512-body override",
        errors,
        count=0,
    )
    _require_rust_source_token_sequence(
        paths["kagami"],
        sources["kagami"],
        """
.write(["network", "max_total_connections"],
""",
        "Kagami must inherit the shared Core source cap instead of diverging locally",
        errors,
        count=0,
    )

    for role, name in (
        (
            "actual",
            "sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
        ),
        (
            "user",
            "sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
        ),
        (
            "user",
            "sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
        ),
        (
            "user",
            "sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
        ),
    ):
        found = len(rust_items(sources[role], name))
        if found != 1:
            errors.append(
                f"lifecycle-capacity reviewed test {name} must occur exactly once; found {found}"
            )

    return errors
