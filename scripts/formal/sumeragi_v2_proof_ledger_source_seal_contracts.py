# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _retired_sidecar_gate_ttl_source_errors(
    path: Path,
    source: str,
    role: str,
) -> list[str]:
    """Reject any server-request gate TTL identifier in executable Rust."""

    retired_ttl_tokens = sorted(
        {
            token
            for token in rust_code_tokens(source)
            if "ttl" in token.lower()
            and all(
                fragment in token.lower()
                for fragment in ("server", "request", "gate")
            )
        }
    )
    if not retired_ttl_tokens:
        return []
    return [
        f"{path}: retired wall-clock sidecar gate TTL must remain absent "
        f"from production; found identifiers {retired_ttl_tokens} in the "
        f"{role} seam"
    ]


_SAME_ROUND_SEMANTIC_KERNEL_SOURCE_SHA256 = {
    "crates/iroha_core/src/sumeragi/v2_core/refinement.rs": (
        "fc3c131df1f3511f80ce47720ab87043106239d332936f240348d3244550835e"
    ),
    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs": (
        "2b6c2f9e6ef9b3979ce9f3fd5b5869adf92e2832ee5b9d1c8a9e45876f5a750d"
    ),
    "crates/iroha_core/src/sumeragi/v2_core/types.rs": (
        "30ec760fa27f2d7ee59c716570f2af457aa005a8377ee13ea8c17634b35699b8"
    ),
    "crates/iroha_core/src/sumeragi/v2_core/wal.rs": (
        "6bff6e8e90983f8bd1657de5faaf59b5db9a57e99ba0f9e0be96e7de0d3e2b9f"
    ),
    "crates/iroha_core/src/sumeragi/v2_effects.rs": (
        "18e8ae9a15a2da81a0db130abd1d9a662035881a6f27931f8ed11187346db4a3"
    ),
    "crates/iroha_core/src/sumeragi/v2_runner.rs": (
        "eba5129f19c0dc9e5196f6e888172b7ac52dc59a767370c526d244c198a10542"
    ),
    "crates/iroha_core/src/sumeragi/v2_worker.rs": (
        "0d802c3c32b2bb13cf6369832dd65a4d79360dbc8035b5dd2ad26a86ca78fd88"
    ),
    "crates/iroha_sumeragi_core/src/verus_proofs.rs": (
        "6d492a52dd530130150a22eabec78375929d9431200ed06999d50490053a92a4"
    ),
}


_KURA_PRODUCTION_COMPONENT_FILES = (
    "kura/startup_finality_support.rs",
    "kura/bound_progress_and_retained_support.rs",
    "kura/autonomous_reservation_bounds.rs",
    "kura/prune_commit_merge_support.rs",
    "kura/replica_advert_and_body_status.rs",
    "kura/retained_finality_replica_authority.rs",
    "kura/autonomous_merge_bundle_support.rs",
    "kura/autonomous_reservation_types.rs",
    "kura/autonomous_reservation_inventory.rs",
    "kura/autonomous_reservation_classifier.rs",
    "kura/historical_autonomous_recovery.rs",
)

_REVIEWED_RUST_INCLUDE_MANIFESTS = {
    "crates/iroha_config/src/parameters/actual.rs": (
        "actual/tests.rs",
    ),
    "crates/iroha_config/src/parameters/user.rs": (
        "user/kura.rs",
        "user/kura_and_snapshot_tests.rs",
    ),
    "crates/iroha_core/src/kura.rs": (
        *_KURA_PRODUCTION_COMPONENT_FILES,
        "kura/tests/01_support_snapshot_bootstrap_and_rewrite.rs",
        "kura/tests/01a_retained_eviction_and_rewrite_tail.rs",
        "kura/tests/02_replacement_and_preflight.rs",
        "kura/tests/03_preflight_and_merge_entry.rs",
        "kura/tests/03a_preflight_and_merge_entry_tail.rs",
        "kura/tests/04_merge_log_and_associations.rs",
        "kura/tests/05_merge_resolution_and_eviction.rs",
        "kura/tests/05a_replica_advert_and_body_eviction.rs",
        "kura/tests/06_eviction_and_autonomous_lanes.rs",
        "kura/tests/07a_autonomous_reservation_reconciliation_support.rs",
        "kura/tests/07_autonomous_lanes_and_sidecars.rs",
        "kura/tests/07b_autonomous_reservation_reconciliation_tests.rs",
        "kura/tests/08_lane_receipts_and_artifacts.rs",
        "kura/tests/09_lane_artifacts_and_fastpq.rs",
        "kura/tests/10_native_amx_and_roster.rs",
        "kura/tests/10b_native_amx_prepublication_transition.rs",
        "kura/tests/11_roster_and_progress_sidecars.rs",
        "kura/tests/12_sidecar_index_and_pruning.rs",
        "kura/tests/13_manifests_and_fsync.rs",
    ),
    "crates/iroha_core/src/kura/lane_geometry.rs": (
        "lane_geometry_tests/00_support.rs",
        "lane_geometry/native_amx_retained_window_tests.rs",
        "lane_geometry_tests/00_retirement.rs",
        "lane_geometry_tests/01_retirement_and_recovery.rs",
        "lane_geometry_tests/02_geometry_moves_and_journal.rs",
        "lane_geometry_tests/03_gc_and_startup.rs",
    ),
    "crates/iroha_core/src/snapshot.rs": (
        "snapshot/support_policy_tests.rs",
        "snapshot/write_roundtrip_tests.rs",
        "snapshot/reconciliation_generation_tests.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_worker.rs": (
        "v2_worker/exact_output_rollover_claim.rs",
        "v2_worker/kura_replica_advert_refresh.rs",
        "tests/v2_worker_reply_route_cases.rs",
        "tests/v2_worker_backpressure_cases.rs",
        "tests/v2_worker_serve_unsealed_cases.rs",
        "tests/v2_worker_serve_decision_restart_cases.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_runtime.rs": (
        "tests/v2_runtime_unsealed_00.rs",
        "tests/v2_runtime_unsealed_01.rs",
        "tests/v2_runtime_unsealed_02.rs",
        "tests/v2_runtime_unsealed_03.rs",
        "tests/v2_runtime_unsealed_04.rs",
        "tests/v2_runtime_unsealed_05.rs",
        "tests/v2_runtime_unsealed_06.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_runner.rs": (
        "tests/v2_runner_unsealed_00.rs",
        "tests/v2_runner_unsealed_01.rs",
        "tests/v2_runner_unsealed_02.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_apply.rs": (
        "tests/v2_apply_unsealed_00.rs",
        "tests/v2_apply_unsealed_01.rs",
        "tests/v2_apply_unsealed_02.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/reducer.rs": (
        "tests/v2_core_reducer_primitive_projection.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_core/tests.rs": (
        "tests/v2_core_view_zero_parent_binding.rs",
        "tests/empty_replay_resume_test.rs",
    ),
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs": (
        "v2_lane_work/canonical_executed_block_application_repair.rs",
        "tests/v2_lane_work_observer_role.rs",
        "tests/v2_lane_work_native_body_recovery.rs",
        "tests/v2_lane_work_effect_queue.rs",
    ),
}


def _read_reviewed_rust_source(
    repo_root: Path,
    relative: str,
    errors: list[str],
    description: str,
) -> tuple[Path, str]:
    """Read one Rust source after authenticating and expanding its include closure."""

    path = repo_root / relative
    if not path.is_file() or path.is_symlink():
        errors.append(f"{path}: {description} must be a regular non-symlink file")
        return path, ""
    try:
        source = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{path}: cannot read {description}: {error}")
        return path, ""

    manifest = _REVIEWED_RUST_INCLUDE_MANIFESTS.get(relative)
    if manifest is None:
        return path, source

    masked_source = mask_rust_comments(source)
    include_invocations = tuple(
        re.finditer(r"(?m)^[ \t]*include\s*!", masked_source)
    )
    include_pattern = re.compile(
        r'(?m)^[ \t]*include\s*!\s*\(\s*"'
        r'(?P<relative>[^"\n]+\.rs)"\s*\)\s*;[ \t]*(?:\n|$)'
    )
    observed = tuple(
        match.group("relative") for match in include_pattern.finditer(masked_source)
    )
    if observed != manifest or len(include_invocations) != len(manifest):
        errors.append(
            f"{path}: reviewed Rust include inventory must equal {manifest!r}; "
            f"found {observed!r} across {len(include_invocations)} include "
            "invocation(s)"
        )

    component_sources: dict[str, str] = {}
    for component_relative in manifest:
        component_path = path.parent / component_relative
        if not component_path.is_file() or component_path.is_symlink():
            errors.append(
                f"{component_path}: reviewed Rust include component for {path} "
                "must be a regular non-symlink file"
            )
            component_source = ""
        else:
            try:
                component_source = component_path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as error:
                errors.append(
                    f"{component_path}: cannot read reviewed Rust include "
                    f"component for {path}: {error}"
                )
                component_source = ""
        component_sources[component_relative] = component_source

    def expand(match: re.Match[str]) -> str:
        component_relative = match.group("relative")
        component_source = component_sources.get(component_relative)
        return match.group(0) if component_source is None else component_source

    return path, include_pattern.sub(expand, source)


def _reviewed_rust_include_manifest_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Fail closed unless every reviewed split Rust source has its exact closure."""

    errors: list[str] = []
    for relative in _REVIEWED_RUST_INCLUDE_MANIFESTS:
        _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "reviewed split Rust source",
        )
    return errors


def _kura_production_source_inventory(
    repo_root: Path = ROOT_DIR,
) -> tuple[
    Path,
    str,
    tuple[tuple[str, Path, str], ...],
    list[str],
]:
    """Load the exact direct production include closure of ``kura.rs``."""

    source_root = repo_root / "crates" / "iroha_core" / "src"
    kura_path = source_root / "kura.rs"
    errors: list[str] = []
    if not kura_path.is_file() or kura_path.is_symlink():
        errors.append(
            f"{kura_path}: Kura production source inventory root must be a "
            "regular file"
        )
        kura_source = ""
    else:
        try:
            kura_source = kura_path.read_text(encoding="utf-8")
        except OSError as error:
            errors.append(
                f"{kura_path}: cannot read Kura production source inventory "
                f"root: {error}"
            )
            kura_source = ""

    masked_source = mask_rust_comments(kura_source)
    test_module_markers = tuple(
        re.finditer(
            r"(?m)^#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*\n"
            r"mod\s+tests\s*\{",
            masked_source,
        )
    )
    if len(test_module_markers) != 1:
        errors.append(
            f"{kura_path}: Kura production source inventory must retain "
            "exactly one terminal cfg(test) module boundary; found "
            f"{len(test_module_markers)}"
        )
        production_source = masked_source
    else:
        production_source = masked_source[: test_module_markers[0].start()]

    include_invocations = tuple(
        re.finditer(r"(?m)^[ \t]*include\s*!", production_source)
    )
    include_pattern = re.compile(
        r'(?m)^[ \t]*include\s*!\s*\(\s*"'
        r'(?P<relative>kura/[^"\n]+\.rs)"\s*\)\s*;[ \t]*$'
    )
    observed_components = tuple(
        match.group("relative")
        for match in include_pattern.finditer(production_source)
    )
    if (
        observed_components != _KURA_PRODUCTION_COMPONENT_FILES
        or len(include_invocations) != len(_KURA_PRODUCTION_COMPONENT_FILES)
    ):
        errors.append(
            f"{kura_path}: Kura direct production include inventory must equal "
            f"{_KURA_PRODUCTION_COMPONENT_FILES!r}; found "
            f"{observed_components!r} across {len(include_invocations)} "
            "include invocation(s)"
        )

    components: list[tuple[str, Path, str]] = []
    for relative in _KURA_PRODUCTION_COMPONENT_FILES:
        path = source_root / relative
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: Kura production source inventory component must be "
                "a regular non-symlink file"
            )
            source = ""
        else:
            try:
                source = path.read_text(encoding="utf-8")
            except OSError as error:
                errors.append(
                    f"{path}: cannot read Kura production source inventory "
                    f"component: {error}"
                )
                source = ""
        components.append((relative, path, source))
    return kura_path, kura_source, tuple(components), errors


# Exact token-stream digests for the nontrivial Verus projection theorems and
# their concrete mutation witnesses. Unlike raw substrings, these bind the
# declaration, contract, and proof body of one real, context-checked item while
# remaining insensitive to comments, literals, and formatting.
_PRODUCTION_CAUSAL_FIFO_VERUS_ITEM_SHA256 = {
    "production_stable_subsequence": (
        "820ed299168153274ca3457e3446bd8bd2c433b49286bd9f5e4c57975af252ba"
    ),
    "production_fresh_causal_successors_excludes_prior_owners": (
        "b6236603215e29cabfc1a44d7495ad101d0390f23863127714354bb2d89b0c56"
    ),
    "production_fresh_causal_successors_keeps_every_fresh_value": (
        "0ff2376b08c3a96cb0c82f3d87404796a4d863f919c9cf58ddee5858c3ebf898"
    ),
    "production_fresh_causal_successors_has_unique_values": (
        "030db5f75b6cbddc32ba57aa8eac66169b993b3b067105232005d0985ee83daa"
    ),
    "production_fresh_causal_successors_preserves_first_owner_order": (
        "14ef7781949552882dda3fb35f066ea89e0a943bb40dcca2b7464b0a05d81109"
    ),
    "production_async_causal_fifo_after_batch_preserves_fresh_tail": (
        "ef11350ee44c0551a2a34f091f3197559a684075b3c86a28d0d4a9528acc6a6e"
    ),
    "production_inverted_owner_filter_mutant": (
        "550fa68ae21e42cf9bd1c9756e229ed40b23e28ebc417cd7907d9b805e634bec"
    ),
    "production_inverted_owner_filter_mutant_is_rejected": (
        "898288a0767a678bd7080abe09995de2ec2e7c406c796ab79202b28e200d0a41"
    ),
    "production_reversed_fresh_order_mutant": (
        "e14bf30fb8489b4922e8f81c79e305a0f2c07d57f2abd3101bff581c915760a7"
    ),
    "production_reversed_fresh_order_mutant_is_rejected": (
        "a0b995d9442eb0fef330eb7383c258f16dab35b61b84e02716437f758214a6f5"
    ),
}

# Exact comment/literal-free token digests for the production Rust items whose
# control flow establishes the bounded persisted-continuation, one-transition
# deferred scheduling, and TC recovery seam.
# These whole-item bindings prevent an attacker from preserving the reviewed
# snippets in a dead branch while changing the executable path around them.
_PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256 = {
    "from_record": (
        "399605c9add8c3bf579fc03c6b46b52533c1ca726cc175fcd80cd33536882ce7"
    ),
    "budget": (
        "8da4eaa39048654d522ba93b3549224a5f478a8f5e054956c1f82f4d30d2b697"
    ),
    "deferred_work_is_serviceable": (
        "1036f93b127acad65512fcb095d7f19bdc547f93d22ebebbd227277a0a30405c"
    ),
    "completion_unblocks_deferred_fence": (
        "0964e3e5d76ef3f72a240c03c46c3c15a71c52572acaa34784276c97858d3ab7"
    ),
    "command_is_blocked_by_deferred_fence": (
        "05f8139fb6d9566a2749cc01e8e906c180ebca304448d635f2d276a5935f3757"
    ),
    "authenticated_command_reaches_fenced_reducer": (
        "9a4aa6932087f18a3b4897e3946bb40a9e600c99fd4841ebce7a801247ee9835"
    ),
    "ready_to_finish": (
        "039df6143d8b4505489db3c75ab2d3d24c01a30a4da0f0d277d865f44cfa0ffa"
    ),
    "drain_deferred_with_evidence": (
        _SERVICED_CANDIDATE_PRODUCTION_ITEM_SHA256[
            "drain_deferred_with_evidence"
        ]
    ),
    "fail_deferred_service_contract": (
        "b3ca797528869db777b292d74ccba9425cced620e3d56f429ffdeb624b285799"
    ),
    "drive_effects": (
        "98ff14c212f7c5f8b1756e7d8661a3ea502dfc334fc22ca592a7f85958909f29"
    ),
    "pop_fence_completion_with_ownership": (
        "8fc875ae966aef68a30bec6fb163c10895449a367a9d5731e5e3f8c78594bd64"
    ),
    "fence_blocked_lifecycle_owners": (
        "a6ab663afdf61312dd4da128d54967c262f749f966a953dc63abc77d3751c83d"
    ),
    "freeze_due_clock_owners": (
        "f097b1ce583dcb9d94293366cab9828f648c376dd586f7af28635b003f92c855"
    ),
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "204a88b77ef7853327a2313583c25ef32f9bdea705dd57e0fc7bbd17b30afa1a"
    ),
    "runtime_step": (
        "82e349d880d8ff39c7438e8c06b301f5abcae332d25265285213a099b7ad8ce3"
    ),
    "runtime_step_recovery": (
        "1e3243e36d27d9d0fb510048485350c2401d56ba02f1ef274b027c63e06847bd"
    ),
    "dispatch_one_adapter_deferred": (
        "20d4d698e8566e7df3ba96d0f6cc136ad87c05f10d95931b886aa174ae45b7b9"
    ),
    "dispatch_one_fence_dependency": (
        "1109cb4b250d3115032fc5d5ed196182b53b9020ea4142f0f87e06b197731f00"
    ),
    "real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo": (
        "7f93137f29f8c8afcf06e45935c979dc10ad786b88e87f617415adc67df0d242"
    ),
    "real_adapter_fence_completion_breaks_pre_and_post_timeout_retransmit_debt": (
        "89324b884105cbcb609cf18945c41719ff47edeaf315ad3549b316ea0d0744e7"
    ),
    "tc_promoted_lock_requires_same_subject_reproposal_before_commit": (
        "4a6ae3cd80c629e1ec63e32eadc6d6cfcb68a9ccc77a15997a4915931818279c"
    ),
}

# Exact comment/literal-free token digests for the production ownership bridge
# which retains the complete canonical envelope and fair-ingress carrier from
# authenticated admission through Busy-deferred service.
_AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256 = {
    "matches_authenticated_runtime_bytes": (
        "1d5bd7b516504865f7f8f8db0416c7c1d337ba83342cbd66b564b24e46df3870"
    ),
    "deferred_authenticated_message_owner": (
        "b8279dae9cd51a72cc4a84cd80063fc5a90b2ca72066b48f59571166b560cc8b"
    ),
    "authenticated_deferred_admission_ordinals": (
        "56e2d8e09f770616ace0c2421e6105c7ee2cf8d7428b4016003b5aa5b71bf271"
    ),
    "deferred_authenticated_event_matches_wire": (
        "71cff12249ba75d45cc55f3be85c966fa2f317a3638ce36fa250d399c0f88fd5"
    ),
    "wire_ingress_missing_execution_commitment": (
        "ab4345a4067a48f67735cb5867d717cf8f32aa809f7218aeb04c8eaaf3775678"
    ),
    "runtime_ingress_from_fair_ingress": (
        "638d44eae201d3477a987e857d3c9318c7a525347cddb9d5bbce704d9bbc7985"
    ),
    "runtime_ingress_validate_exact": (
        "5076de66c0c2a3b01f31c65ff4ea3af8177e4bd372861d6e14585087fb6714d3"
    ),
    "runtime_ingress_matches_authenticated": (
        "e9d5c8bfa0cbdc71f42f9858ee00e73f663c2bed53f573c3e06a9144feaad5f8"
    ),
    "runtime_ingress_can_merge_downstream": (
        "0a9b6424c76b8d53a6e630431e3e9153d20e25d755abbe38384c5f59e9064553"
    ),
    "runtime_ingress_merge_downstream": (
        "55f59286a356c0cd1c47b572db72660dcdedcc40f3946c2e3b62c7eb5c805e92"
    ),
    "runtime_driver_dispatch": (
        "a20f3f9675cacba345a174bba5fa8370f0f76225f8e3e040971c2d7f15800990"
    ),
    "runtime_driver_dispatch_deferred": (
        "f80c271a6766106b620e6b0dcbd7fb3a37db63fece634c09818ddb74498202d9"
    ),
    "reconcile_deferred_ingress_ownership": (
        "fb8ac9defb961830528e67b3c05e92e7a72368c4c592f5594285f937cfb81f01"
    ),
    "accept_driver_dispatch": (
        "2862df8110e056352a64680616e131393c4eab44be5f9e091f43ccc23d6fab65"
    ),
    "enqueue_network_with_ingress_ownership": (
        "db05791e686e2c8b4a84e09707c83b767f3bc7eef1da189060801cf86dfc5927"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "24903fdc43ff249788662132bdf1c260917a8243b260e67ba5e8224a013d361d"
    ),
    "take_last_scheduler_ownership": (
        "b781f7ace9823e4ba2b395230912a703a78c2b6ae8fb48e96a0f0f120c9fa7c8"
    ),
    "commit_certificate_response_coalesces_with_exact_busy_deferred_qc": (
        "29cc0769d5f9224ea11e10410b05b97a4e33fb75b9327ee4a18f19e12babbfe9"
    ),
}

_PRODUCTION_CAUSAL_FIFO_RUNTIME_REGRESSION_SHA256 = {
    "later_same_semantic_fair_retry_retains_runtime_lifecycle_root": (
        "12e41e90bcd59ef22389131ad4b5cdeb56dce7fa04cd15357d18ce9f62cb1b6a"
    ),
    "ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it": (
        "b69ec152b545fb4e705335eeaecaa5785b1736349fa1494b383f18c257aa8d18"
    ),
    "older_frozen_aggregate_carrier_rebases_queued_runtime_minimum": (
        "dc5f365edfc9bbd31d0f8f595c34673a779e85aac7dfe517316792bd2c70a235"
    ),
    "network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals": (
        "878269e9bdf567147360a1f3c2bf5a7def18b7abd1b38aed3978dc7923624291"
    ),
}

# Exact production effect-executor entry points which retain the authenticated
# ingress carrier and latch a fail-stop restart on malformed ownership.
_EFFECT_CAPACITY_PRODUCTION_RUST_ITEM_SHA256 = {
    "enqueue_network_with_ingress_ownership": (
        "9bf3e8ec45247e920681de7f13941d8544df863b73c5a3ac243adcacae0b2587"
    ),
    "can_admit_network_message_with_ingress_ownership": (
        "644421658388af9adb80656b4b0d7b402835187c13d5c32b92e43f41ae5ff10f"
    ),
}

# The retained exact Fetch lifecycle must not duplicate service work while its
# immutable owner is already installed, and completion must publish the
# reserved runtime successor before retiring any local owner.  Bind both full
# methods in addition to the focused ordering checks below.
_EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256 = {
    "begin_fetch": (
        "7b34ba0db72d22ad1e86935f129bbd35d17aa31888b67df601c9865cfbd9eb11"
    ),
    "commit_fetch_completion": (
        "ee63eedba56be13b1af788f81f9f69dc380a834bc0a8d0b36e1de3910f041f92"
    ),
}

# Exact adversarial integration witnesses for the durable locked-Commit owner
# alternatives.  These tests are release inventory, not deductive evidence;
# whole-item seals prevent a renamed or weakened lookalike from satisfying the
# progress-witness regression contract.
_LOCKED_COMMIT_PROGRESS_WITNESS_TEST_SHA256 = {
    "locked_commit_progress_witness_rejects_inexact_or_empty_ownership": (
        "a40965bfa911b0f8b2cf118644aaf07d4cd6898246b1d5b72b9ea4e15649a9d6"
    ),
    "locked_commit_progress_witness_accepts_each_exact_owner": (
        "07823a34cefc84c62880027df0898fd5524c5e1e7f9ec71e0ebc84d7998e45b7"
    ),
}
_LOCKED_COMMIT_PROGRESS_WITNESS_HELPER_SHA256 = {
    "locked_commit_has_exact_progress_witness": (
        "e63115adfa82478faa975238d72973dbe84791f45fb8c968732d62974d2c44f4"
    ),
    "validate_locked_commit_progress_witness": (
        "6a3d419be24f3266f2a38b71cfb79c5431d127925dd133416723c5890da200e0"
    ),
}

_PRODUCTION_LIVENESS_RELEASE_COUNT = 806
_PRODUCTION_LIVENESS_RELEASE_CORRIDOR_LEG_COUNT = 82
_PRODUCTION_LIVENESS_RELEASE_INVENTORY_SHA256 = (
    "0c1ee4be74b736dba126c0eeedd1f39ccaf6cbd6eed0e6374f158e3457f2eff5"
)
_CLOSED_SIDECAR_PREFIX_HANDOFF_TEST_SHA256 = (
    "75019365bd62839da229b51671071af1b9165f4c08fc06d36be6bc2e4e14b893"
)
_PRODUCTION_MULTILANE_FOCUS_TEST_COUNT = 390
_PRODUCTION_MULTILANE_G_UNIT_TSV_LINE_COUNT = 391
_PRODUCTION_MULTILANE_FOCUS_INVENTORY_SHA256 = (
    "1ea0b8f3ebf914e7aab0a1a680450e7decfbcf20242243580a94e17de456fcf4"
)
_PRODUCTION_MULTILANE_FOCUS_CONTRACTS = (
    (
        "required_multilane_core_focus_tests",
        "g-unit-iroha-core",
        "iroha_core",
    ),
    (
        "required_multilane_queue_journal_focus_tests",
        "g-unit-iroha-core-queue-journal",
        "iroha_core",
    ),
    (
        "required_multilane_config_lib_focus_tests",
        "g-unit-iroha-config-lib",
        "iroha_config",
    ),
    (
        "required_multilane_config_runtime_focus_tests",
        "g-unit-iroha-config-runtime",
        "iroha_config",
    ),
    (
        "required_multilane_config_fixtures_focus_tests",
        "g-unit-iroha-config-fixtures",
        "iroha_config",
    ),
    (
        "required_multilane_data_model_focus_tests",
        "g-unit-iroha-data-model",
        "iroha_data_model",
    ),
    (
        "required_multilane_torii_focus_tests",
        "g-unit-iroha-torii",
        "iroha_torii",
    ),
    (
        "required_multilane_torii_shared_focus_tests",
        "g-unit-iroha-torii-shared",
        "iroha_torii_shared",
    ),
    (
        "required_multilane_integration_lib_focus_tests",
        "g-unit-integration-tests",
        "integration_tests",
    ),
)
_GENESIS_HEADER_BINDING_TEST_SHA256 = (
    "8d847d27cdea09a87f5ee4ec940f60f9fa73fb85ca9a965d2a3fcac19eb3b41e"
)
_RESTART_VIEW_ZERO_DEADLINE_TEST_SHA256 = (
    "13c1cd988856a8c4ee4d20cfc176c4111352ba7262d07bb417de5a4056cf8b1f"
)
_SUCCESSOR_PARENT_BINDING_TEST_SHA256 = {
    "successor_core_context_preserves_the_parent_certificate_binding": (
        "79c2caea8dfd6f17885ff3d72253a41cb34db7a99d7976b52d5fdab45c0e9a89"
    ),
    "successor_context_requires_the_durable_cryptographic_parent": (
        "07aa14d187145445218084edfccf2c0675cb97b10969f58065b75c9567779cd6"
    ),
    "authentication_rejects_valid_commitment_conflicts_without_mutating_adapter": (
        "414c5f5bf9c7156f38222f256673c42e1ce2394293193487fe9d5d2d24044286"
    ),
}
_LATE_LANE_RECOVERY_TEST_SHA256 = (
    "ae4bca0b785e6d7d5db41dd5516ce2859464f0f0526d5d7d3a52a3c930f6025e"
)
_PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS = (
    ("production-kura-progress-durability", "kura::tests", 13),
    ("production-kura-lane-geometry", "kura::lane_geometry::tests", 8),
    ("production-lane-relay-exact-ownership", "nexus::lane_relay::tests", 4),
    (
        "production-authoritative-ingress",
        "sumeragi::authoritative_runtime_gate_tests",
        40,
    ),
    ("production-merge-sidecar", "merge_sidecar::tests", 118),
    ("production-v2-core", "sumeragi::v2_core::tests", 38),
    ("production-v2-core-refinement", "sumeragi::v2_core::refinement::tests", 17),
    (
        "production-v2-core-wal",
        "sumeragi::v2_core::wal::byte_lifecycle_tests",
        1,
    ),
    (
        "production-v2-core-source-link",
        "sumeragi::v2_core::reducer::source_link_tests",
        8,
    ),
    (
        "production-v2-equivocation-evidence",
        "sumeragi::evidence::tests",
        1,
    ),
    (
        "production-v2-leader-wire-lifecycle-store",
        "sumeragi::serviced_candidate_store::tests",
        1,
    ),
    ("production-v2-adapter", "sumeragi::v2::tests", 45),
    ("production-v2-body-store", "sumeragi::v2_body_store::tests", 2),
    ("production-v2-block-sync", "sumeragi::v2_block_sync::tests", 3),
    ("production-v2-apply", "sumeragi::v2_apply::tests", 1),
    ("production-v2-effects", "sumeragi::v2_effects::tests", 66),
    ("production-v2-lane-work", "sumeragi::v2_lane_work::tests", 53),
    ("production-v2-runtime", "sumeragi::v2_runtime::tests", 52),
    ("production-v2-transport", "sumeragi::v2_transport::tests", 1),
    ("production-v2-recovery", "sumeragi::v2_recovery::tests", 3),
    ("production-v2-runner", "sumeragi::v2_runner::tests", 34),
    ("production-v2-worker", "sumeragi::v2_worker::tests", 129),
    (
        "production-v2-watchdog",
        "sumeragi::status::v2_liveness_watchdog_tests",
        19,
    ),
    (
        "production-kagemusha-finality",
        "zk::kagemusha_finality::tests",
        1,
    ),
    (
        "production-data-model-v2-finality",
        "block::consensus_v2::finality::tests",
        1,
    ),
    (
        "production-data-model-offline-compact-qc",
        "offline::kagemusha_v4_topup_provenance_tests",
        1,
    ),
    (
        "production-data-model-v2-context-identity",
        "block::consensus_v2::tests",
        2,
    ),
    ("production-v2-integration-runner", "sumeragi_v2_runner", 4),
    ("production-p2p-peer-reliable-flush", "peer::run::tests", 11),
    (
        "production-p2p-shared-source-byte-geometry",
        "peer::shared_byte_budget_tests",
        8,
    ),
    ("production-p2p-network-reliable-actor", "network::tests", 84),
    (
        "production-p2p-source-memory-geometry",
        "network::inbound_source_memory_bound_tests",
        2,
    ),
    (
        "production-p2p-waiter-rank-geometry",
        "network::handle_update_tests",
        4,
    ),
    (
        "production-irohad-consensus-message-control",
        "consensus_message_control::tests",
        8,
    ),
    ("production-irohad-network-relay", "network_relay_tests", 4),
    ("production-irohad-authenticated-via", "tests::relay_fairness", 7),
    (
        "production-irohad-genesis-reply-geometry",
        "genesis_bootstrap::tests",
        5,
    ),
    (
        "production-config-v2-exact-output-geometry",
        "parameters::actual::tests",
        2,
    ),
    (
        "production-config-v2-exact-output-root-parse",
        "parameters::user::duration_clamp_tests",
        5,
    ),
)
_PRODUCTION_LIVENESS_RELEASE_MODULES = tuple(
    module for _, module, _ in _PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = (
    "kura::tests::certified_lane_block_encoding_enforces_source_envelope",
    "nexus::lane_relay::tests::actor_backpressure_retains_exact_relay_and_fifo_ticket",
    "nexus::lane_relay::tests::blocked_relay_does_not_starve_a_responsive_relay",
    "nexus::lane_relay::tests::terminal_actor_failures_return_exact_relay_ownership",
    "nexus::lane_relay::tests::saturated_relay_owner_returns_sixty_fifth_without_actor_ticket",
    "sumeragi::authoritative_runtime_gate_tests::direct_and_synthetic_envelopes_keep_identity_roles_consistent",
    "sumeragi::authoritative_runtime_gate_tests::atomic_lane_certificate_uses_the_shared_progress_owner",
    "sumeragi::authoritative_runtime_gate_tests::oversized_atomic_lane_certificate_is_returned_exactly",
    "sumeragi::authoritative_runtime_gate_tests::relayed_origin_churn_uses_one_via_lane_and_preserves_protocol_origin",
    "sumeragi::authoritative_runtime_gate_tests::authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains",
    "sumeragi::authoritative_runtime_gate_tests::roster_origin_relay_completion_has_authenticated_source_count_and_byte_owner",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_wire_index_keeps_authenticated_origins_distinct",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure",
    "sumeragi::serviced_candidate_store::tests::leader_wire_gate_retains_independent_cross_origin_phase_and_chunk_slots",
    "sumeragi::v2_effects::tests::effect_dispatch_consumes_leader_wire_terminal_created_while_batch_drains",
    "sumeragi::v2_effects::tests::retained_live_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_effects::tests::retained_recovery_retry_consumes_decision_retirement_terminal_same_cycle",
    "sumeragi::v2_runtime::tests::decision_retirement_releases_queued_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::lock_retirement_releases_busy_deferred_leader_wire_runtime_owner",
    "sumeragi::v2_runtime::tests::production_authenticated_preflight_is_never_semantic_only_coalesce",
    "sumeragi::v2_runtime::tests::semantic_only_authenticated_coalesce_fails_before_receipt_registration",
    "sumeragi::v2_runner::tests::fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes",
    "sumeragi::authoritative_runtime_gate_tests::alternate_reply_route_attaches_before_authenticated_source_lane_cap",
    "sumeragi::authoritative_runtime_gate_tests::transport_reply_route_construction_is_fallible_and_target_bound",
    "consensus_message_control::tests::stale_duplicate_reordered_and_unknown_releases_are_atomic",
    "consensus_message_control::tests::hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic",
    "consensus_message_control::tests::drain_fence_holds_racing_chunks_fifo_until_atomic_cutover",
    "tests::relay_fairness::hold_release_preserves_exact_layered_ownership_until_recorded_terminal",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_must_fit_network_geometry",
    "parameters::user::duration_clamp_tests::sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry",
    "parameters::actual::tests::sumeragi_v2_config_format_changes_the_handshake_fingerprint",
    "sumeragi::v2_core::refinement::tests::historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution",
    "sumeragi::v2_core::refinement::tests::historical_certificate_kernel_rejects_foreign_admission_and_unretired_request",
    "peer::run::tests::consensus_lane_and_v2_topics_share_authenticated_high_source_credit",
    "merge_sidecar::tests::exact_active_delivery_retry_preserves_decreasing_chunk_rank",
    "merge_sidecar::tests::alternate_source_progress_and_reconnect_preserve_independent_cursors",
    "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
    "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
    "merge_sidecar::tests::later_delivery_preserves_the_current_source_cursor",
    "merge_sidecar::tests::later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit",
    "merge_sidecar::tests::late_old_exact_item_receipt_completes_reconnected_attempt_once",
    "merge_sidecar::tests::later_delivery_during_materialization_keeps_exact_authorized_route",
    "merge_sidecar::tests::writable_reconnect_during_materialization_keeps_exact_authorized_tenure",
    "merge_sidecar::tests::equal_sequence_with_different_semantic_identity_is_rejected_before_materialization",
    "merge_sidecar::tests::transient_materialization_release_keeps_exact_retry",
    "merge_sidecar::tests::transient_response_capacity_defers_materialization_on_the_same_delivery",
    "merge_sidecar::tests::response_materialization_requires_and_consumes_its_exact_admission_gate",
    "merge_sidecar::tests::sidecar_admission_matches_the_cached_arc_without_changing_ownership",
    "merge_sidecar::tests::inactive_reply_route_is_rejected_before_server_gate_admission",
    "merge_sidecar::tests::completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses",
    "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
    "merge_sidecar::tests::completed_source_does_not_block_a_new_alternate_source",
    "merge_sidecar::tests::configured_route_source_capacity_bounds_semantic_attempts",
    "merge_sidecar::tests::configured_source_geometry_reserves_more_than_eight_independent_attempts",
    "merge_sidecar::tests::third_session_from_one_hub_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::source_byte_overflow_is_rejected_while_another_hub_progresses",
    "merge_sidecar::tests::completed_short_session_replacement_cannot_starve_an_older_long_session",
    "merge_sidecar::tests::route_retirement_between_admission_and_enqueue_releases_all_response_reservations",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_session",
    "merge_sidecar::tests::saturated_materializer_does_not_erase_same_request_alternate_bytes",
    "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
    "merge_sidecar::tests::durable_response_drain_persists_pending_identity_before_handoff",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_is_one_atomic_kura_backed_response",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_serves_rotated_validator_after_pressure",
    "sumeragi::v2_lane_work::tests::historical_certificate_survives_successor_lock_decision_persistence_and_restart",
    "sumeragi::v2_lane_work::tests::carrier_replacement_filters_persistence_and_output_sources_together",
    "sumeragi::v2_lane_work::tests::applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts",
    "sumeragi::v2_lane_work::tests::native_amx_request_rejects_inactive_reply_route_before_signing",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_preserves_exact_source_delivery",
    "sumeragi::v2_lane_work::tests::reply_effect_rejects_missing_or_retargeted_route_set",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_updates_only_later_delivery_from_same_source",
    "sumeragi::v2_lane_work::tests::duplicate_reply_effect_retains_alternate_sources_across_source_update",
    "sumeragi::v2_lane_work::tests::temporarily_unserviceable_effect_requeues_behind_later_reserved_work",
    "sumeragi::v2_lane_work::tests::retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling",
    "sumeragi::v2_runtime::tests::commit_certificate_response_coalesces_with_exact_busy_deferred_qc",
    "peer::run::tests::authenticated_source_credit_precedes_network_and_subscriber_backlogs",
    "peer::run::tests::recoverable_post_acknowledges_only_after_full_write_and_flush",
    "peer::run::tests::partial_write_error_closes_ack_without_false_completion",
    "peer::run::tests::coalesced_batch_acknowledges_every_item_only_after_flush",
    "peer::run::tests::maximum_frame_uses_a_bounded_number_of_source_reservations",
    "peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting",
    "peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift",
    "network::inbound_source_memory_bound_tests::authenticated_source_count_share_is_checked_and_never_zero",
    "network::tests::actor_progress_bypasses_full_deferred_owner_and_waits_for_writer_flush",
    "network::tests::actor_progress_lease_survives_topology_transition",
    "network::tests::actor_progress_retries_exactly_once_on_peer_writer_replacement",
    "network::tests::actor_progress_retry_round_robin_bypasses_partitioned_target",
    "network::tests::cap_one_blocked_source_cannot_prevent_live_source_service",
    "network::tests::actor_progress_lease_survives_debug_packet_loss_until_delivery_retries",
    "network::tests::actor_broadcast_retry_targets_only_failed_peers",
    "network::tests::reliable_subscriber_is_single_consumer_under_clone_budget_pressure",
    "network::tests::reconnecting_peer_cannot_multiply_retained_source_credits",
    "sumeragi::v2_core::refinement::tests::two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations",
    "tests::relay_fairness::daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner",
    "tests::relay_fairness::saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits",
    "tests::relay_fairness::real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress",
    "sumeragi::status::v2_liveness_watchdog_tests::active_watchdog_resets_on_successor_owner_and_status_clear",
    "sumeragi::v2_runner::tests::synthesized_durable_rollover_contract_allows_successor_after_dead_target_handoff",
    "sumeragi::v2_runner::tests::reserved_lane_output_bypasses_unserviceable_head_without_losing_owner",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_durable_lane_certificate_reply_routes",
    "sumeragi::v2_runner::tests::runner_dispatch_preserves_certified_sidecar_chunk_reply_routes",
    "sumeragi::v2_runner::tests::bounded_sidecar_admission_turn_applies_only_its_budget",
    "sumeragi::v2_runner::tests::runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling",
    "sumeragi::v2_runner::tests::runner_dispatch_advances_certified_sidecar_only_after_writer_flush",
    "sumeragi::v2_runner::tests::runner_dispatch_retired_admission_race_emits_no_sidecar_receipt",
    "sumeragi::v2_runner::tests::runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
    "sumeragi::v2_runner::tests::closed_sidecar_prefix_handoff_requeues_only_failed_suffix",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route",
    "sumeragi::v2_runner::tests::runner_dispatch_rejects_durable_response_without_reply_routes",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_exact_final_lane_commit_qc_post",
    "sumeragi::v2_worker::tests::actor_backpressure_retains_complete_merge_share_fanout",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_pending_capacity_replacement",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_rolls_back_materialized_unclaimed_replacement",
    "sumeragi::v2_worker::tests::certified_serve_shutdown_rolls_back_materialized_unclaimed_replacement",
    "sumeragi::v2_worker::tests::certified_serve_terminal_replay_waits_for_barrier_then_bypasses_full_serve_fifo",
    "sumeragi::v2_worker::tests::certified_serve_terminal_replay_source_retains_retired_route_and_reconnects",
    "sumeragi::v2_worker::tests::same_tenure_updates_and_reconnect_preserve_current_item",
    "sumeragi::v2_worker::tests::closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures",
    "sumeragi::v2_worker::tests::completed_sidecar_reconnect_preserves_terminal_cursor_without_capacity_charge",
    "sumeragi::v2_worker::tests::later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
    "sumeragi::v2_worker::tests::inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision",
    "sumeragi::v2_worker::tests::owned_reply_history_merge_retries_candidate_retirement_after_prune",
    "sumeragi::v2_worker::tests::newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source",
    "sumeragi::v2_worker::tests::a_b_a_hub_reconnect_preserves_each_source_cursor",
    "sumeragi::v2_worker::tests::owned_reply_transfer_retirement_after_validation_is_atomic",
    "sumeragi::v2_worker::tests::bulk_backpressure_does_not_block_reserved_lane_or_safety_output",
    "sumeragi::v2_worker::tests::non_roster_targets_cannot_consume_frozen_validator_reservations",
    "sumeragi::v2_worker::tests::partial_fanout_progress_releases_only_the_completed_target_unit",
    "sumeragi::v2_worker::tests::ownership_units_reject_reservation_spill_and_release_exact_target",
    "sumeragi::v2_worker::tests::backpressured_source_does_not_block_other_sources_or_consume_their_reserve",
    "sumeragi::v2_worker::tests::production_output_path_serves_later_fanout_while_target_stays_backpressured",
    "sumeragi::v2_worker::tests::response_outputs_without_exact_routes_fail_stop",
    "sumeragi::v2_worker::tests::sidecar_receipts_use_a_separate_bounded_control_queue",
    "sumeragi::v2_worker::tests::actor_backpressure_cannot_change_returned_payload_identity",
    "sumeragi::v2_worker::tests::exact_output_retry_rejects_a_different_message_identity",
    "sumeragi::v2_worker::tests::full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure",
    "sumeragi::v2_worker::tests::applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
    "sumeragi::v2_worker::tests::applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_output_without_reconstruction",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_unbound_lane_output_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_rejects_wrong_height_global_output",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_historical_kura_global_responses_atomically",
    "sumeragi::v2_worker::tests::applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
    "peer::run::tests::full_write_without_flush_ack_closes_actor_witness_and_retries_on_replacement",
    "network::handle_update_tests::progress_budget_preserves_fifo_for_three_registered_producers",
    "network::tests::reliable_progress_class_matches_actor_reservations_exactly",
    "network::tests::reply_route_survives_peer_message_clone_mapping_and_split",
    "network::tests::peer_message_rehydration_rejects_second_reply_route_without_retargeting",
    "network::tests::reply_source_key_groups_relay_origins_and_orders_actor_instances",
    "network::tests::reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
    "network::tests::dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
    "network::tests::cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
    "network::tests::dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
    "network::tests::reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
    "network::tests::reply_route_binding_rejects_evicted_tombstone_collision",
    "network::tests::reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
    "network::tests::route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
    "network::tests::reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
    "network::tests::reply_actor_admission_does_not_complete_writer_flush_ack",
    "network::tests::reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none",
    "network::tests::retired_reply_tenure_closes_flush_ack_without_false_completion",
    "network::tests::reply_flush_test_fixture_distinguishes_success_timeout_and_close",
    "network::tests::reply_flush_ack_completes_only_after_peer_writer_flush",
    "network::handle_update_tests::progress_ticket_rejects_a_different_same_length_payload",
    "network::tests::configured_assist_hub_connection_cannot_overflow_reliable_geometry",
    "network::tests::topology_larger_than_reliable_target_geometry_is_rejected_atomically",
    "network::tests::assist_hub_refresh_above_reliable_geometry_is_rejected_atomically",
    "network::tests::topology_removal_cancels_every_deferred_owner_for_removed_peer",
    "network::tests::deferred_progress_survives_ttl_but_explicit_peer_removal_cancels_it",
    "network::tests::outside_topology_retransmit_is_not_misreported_as_delivered",
    "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
    "network::handle_update_tests::targetized_broadcast_coalesces_only_the_same_digest_and_membership",
    "network::tests::distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases",
    "network::tests::exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not",
    "network::tests::removed_membership_cancels_only_old_broadcast_debt_across_readd",
    "network::tests::cancelled_target_child_with_pending_flush_ack_releases_exactly_once",
    "network::tests::requested_topology_is_not_authority_and_closed_fanout_returns_all_targets",
    "network::tests::reliable_delivery_waits_for_its_route_subscriber",
    "network::tests::closed_reliable_subscriber_transfers_actor_pending_backlog_to_replacement",
    "network::tests::network_actor_drop_retires_routes_and_only_its_waiters",
    "consensus_message_control::tests::controlled_v2_admission_preserves_distinct_relay_identity",
    "network_relay_tests::test_control_hold_release_preserves_live_route_and_retires_canceled_reentry",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
    "merge_sidecar::tests::sidecar_flush_refinement_advances_only_exact_source_chunk",
    "sumeragi::v2::tests::deferred_actor_source_never_aliases_across_adapter_instances",
    "sumeragi::v2::tests::deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step",
    "sumeragi::v2::tests::deferred_authenticated_retry_retains_exact_original_and_effective_tags",
    "sumeragi::v2::tests::deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap",
    "sumeragi::v2::tests::deferred_service_debt_overflow_is_typed_and_fail_closed",
    "sumeragi::v2::tests::deferred_service_evidence_rejects_every_owner_and_rank_mutation",
    "sumeragi::v2::tests::deferred_zero_ordinal_is_exact_single_use_and_never_reminted",
    "sumeragi::v2_effects::tests::live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks",
    "sumeragi::v2_effects::tests::recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks",
    "sumeragi::v2_lane_work::tests::durable_lane_certificate_coalescing_preserves_alternate_ingress_owners",
    "sumeragi::v2_runtime::tests::adapter_command_identity_is_derived_from_exact_immutable_payload",
    "sumeragi::v2_runtime::tests::admission_ordinal_exhaustion_fails_runtime_closed",
    "sumeragi::v2_runtime::tests::runtime_rejects_replayed_foreign_and_mutated_deferred_tokens",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches",
    "sumeragi::v2_runtime::tests::scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields",
    "sumeragi::v2_runtime::tests::scheduler_owner_must_be_taken_before_a_later_step_can_enter",
    "sumeragi::v2_runtime::tests::selected_owner_without_a_runtime_minted_ordinal_fails_closed",
    "sumeragi::v2_worker::tests::exact_output_coalescing_preserves_distinct_fair_ingress_admissions",
    "sumeragi::v2_worker::tests::orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes",
    "sumeragi::v2_worker::tests::sidecar_flush_ack_identity_mismatch_fails_closed",
    "network::tests::reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence",
    "network::tests::reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor",
    "consensus_message_control::tests::failed_release_clears_in_flight_ownership_and_latches_fatal",
    "consensus_message_control::tests::fatal_controller_rejects_an_unchanged_command_poll",
    "consensus_message_control::tests::retired_release_finishes_drain_without_claiming_delivery",
    "network_relay_tests::obsolete_sumeragi_relay_message_completes_as_delivered",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
    "merge_sidecar::tests::authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes",
    "sumeragi::v2::tests::authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer",
    "sumeragi::v2_effects::tests::owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal",
    "sumeragi::v2_effects::tests::certified_body_response_carrier_swap_fails_closed_before_fetch_mutation",
    "sumeragi::v2_runtime::tests::runtime_merges_alternate_sources_for_one_semantic_request",
    "sumeragi::v2_runtime::tests::runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent",
    "sumeragi::v2_runtime::tests::busy_deferred_request_merges_alternate_source_and_services_exact_carrier",
    "sumeragi::v2_worker::tests::owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors",
    "network::tests::peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures",
    "parameters::actual::tests::sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
    "parameters::user::duration_clamp_tests::sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
    "sumeragi::v2_core::tests::future_view_commit_qc_uses_current_owner_through_application",
    "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
    "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
    "sumeragi::v2_core::tests::stale_generation_completion_is_rejected_after_view_change",
    "sumeragi::v2_core::tests::stale_persistence_completions_stutter_while_current_append_is_pending",
    "sumeragi::v2_core::tests::strictly_ahead_install_timeout_advances_owner_and_protects_highest_prepare",
    "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
    "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
    "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
    "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
    "sumeragi::v2_core::refinement::tests::durable_intent_refinement_accepts_exact_stutters_and_rejects_mutations",
    "sumeragi::v2_core::refinement::tests::locked_commit_progress_witness_accepts_exact_owners_and_rejects_mutations",
    "sumeragi::v2_core::reducer::source_link_tests::certified_fetch_capability_requires_the_exact_proposal_origin",
    "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
    "sumeragi::v2_core::wal::byte_lifecycle_tests::same_round_timeout_replay_accepts_only_a_strict_prepare_origin_upgrade",
    "sumeragi::evidence::tests::sumeragi_v2_equivocation_authenticates_vote_origin_and_execution",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_ownership_projection_ignores_route_liveness_until_maintenance",
    "sumeragi::v2::tests::deferred_projection_distinguishes_authenticated_proposal_origins",
    "sumeragi::v2::tests::vote_body_ownership_uses_the_authenticated_proposal_origin",
    "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
    "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
    "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
    "sumeragi::v2_lane_work::tests::prior_height_hydration_stays_local_under_successor_backpressure",
    "sumeragi::v2_runner::tests::first_same_subject_lock_from_prior_view_retires_unlocked_work",
    "sumeragi::v2_runtime::tests::exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot",
    "sumeragi::v2_runtime::tests::same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically",
    "sumeragi::v2_runtime::tests::runtime_ingress_carrier_capacity_returns_backpressure_atomically",
    "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
    "zk::kagemusha_finality::tests::aggregate_signature_authenticates_proposal_origin",
    "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
    "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
    "offline::kagemusha_v4_topup_provenance_tests::compact_qc_rejects_foreign_or_future_proposal_origin",
    "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
    "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
    "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
    "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
    "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
    "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
)
_PRODUCTION_LIVENESS_RETIRED_REGRESSIONS = frozenset(
    (
        "merge_sidecar::tests::equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
        "merge_sidecar::tests::inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
        "merge_sidecar::tests::partitioned_materialization_preserves_rejected_source_resume_cursor",
        "merge_sidecar::tests::exact_delivery_retry_rematerializes_after_rate_gate_expiry",
        "sumeragi::v2_worker::tests::mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
        "peer::run::tests::dispatch_worker_shutdown_drains_reliable_old_generation_to_actor",
        "network::tests::accepted_draining_generation_delivers_reliable_progress_after_replacement",
        "sumeragi::v2_core::tests::later_view_commit_qc_replays_and_applies_the_retained_lock_origin",
        "sumeragi::v2_core::tests::height_context_rejects_invalid_parent_proposal_origin_geometry",
        "sumeragi::v2_core::tests::same_round_timeout_with_strictly_higher_prepare_rebinds_lock_without_view_change",
        "sumeragi::v2_core::tests::later_lock_and_commit_ack_retires_older_same_origin_commit_pool",
        "sumeragi::v2_core::tests::validated_tc_lock_survives_current_view_timeout_and_commits_after_next_tc",
        "sumeragi::v2_core::tests::replay_resigns_the_newest_commit_intent_for_one_proposal_origin",
        "sumeragi::v2_core::reducer::tests::historical_commit_cannot_cross_the_current_finality_timeout_fence",
        "sumeragi::v2::tests::locked_subject_is_safe_only_at_its_exact_proposal_origin",
        "sumeragi::v2_effects::tests::later_commit_qc_applies_the_exact_retained_lock_origin",
        "sumeragi::v2_effects::tests::later_view_commit_signing_uses_the_fsynced_proposal_origin_marker",
        "sumeragi::v2_transport::tests::later_commit_qc_authenticates_the_exact_locked_body_origin",
        "block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification",
        "block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin",
        "block::consensus_v2::tests::height_context_identity_authenticates_the_parent_proposal_origin",
        "sumeragi_v2_runner::prepare_qc_split_tests::restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
        "sumeragi::v2::tests::successor_core_context_preserves_the_parent_certificate_binding",
        "sumeragi::v2_lane_work::tests::decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
        "sumeragi::v2_recovery::tests::finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
        "sumeragi::v2_runner::tests::terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
    )
)
_PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS = (
    "merge_sidecar::tests::reused_actor_ordinals_under_different_tenures_are_rejected_atomically",
    "merge_sidecar::tests::reply_unwritable_route_parks_inflight_materialization_without_bytes",
    "merge_sidecar::tests::exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon",
    "merge_sidecar::tests::unsent_request_restores_holder_and_backoff_state",
    "merge_sidecar::tests::idle_request_retry_starts_strictly_after_the_fairness_cursor",
    "merge_sidecar::tests::request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix",
    "merge_sidecar::tests::authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression",
    "merge_sidecar::tests::rejected_request_does_not_consume_server_stream_state",
    "merge_sidecar::tests::height_rollover_retries_only_each_sources_current_in_flight_chunk",
    "merge_sidecar::tests::durable_requester_restart_advances_sequence_and_carries_close_floor",
    "merge_sidecar::tests::durable_requester_crash_before_send_closes_unobserved_sequence",
    "merge_sidecar::tests::durable_stream_epochs_and_service_generations_bound_peer_churn",
    "merge_sidecar::tests::durable_lifecycle_rejects_canonical_payload_with_stale_digest",
    "merge_sidecar::tests::durable_responder_restart_preserves_same_hub_gate_budget",
    "merge_sidecar::tests::durable_responder_restart_allows_new_source_while_recovered_source_is_offline",
    "merge_sidecar::tests::durable_responder_restart_preserves_terminal_source_cursor_and_rebinds_capability",
    "sumeragi::v2_lane_work::tests::sidecar_lifecycle_journal_failure_latches_restart_before_request_dispatch",
    "sumeragi::v2_lane_work::tests::sidecar_close_journal_failure_latches_restart_and_blocks_queued_chunk",
    "sumeragi::v2_lane_work::tests::sidecar_close_ack_journal_failure_latches_restart_before_completion",
    "sumeragi::v2_lane_work::tests::sidecar_timeout_journal_failure_latches_restart_before_retry_dispatch",
    "sumeragi::v2_worker::tests::delayed_old_tenure_delivery_cannot_replace_newer_worker_reply_route",
    "sumeragi::v2_worker::tests::ordinary_reply_timeout_grows_only_its_source_attempt_while_sibling_progresses",
    "sumeragi::v2_worker::tests::ordinary_reply_late_old_flush_after_reconnect_advances_exactly_once",
    "sumeragi::v2_worker::tests::mixed_source_retry_retains_pending_flush_target_without_resetting_live_siblings",
    "peer::run::tests::dispatch_worker_shutdown_drains_reliable_replaced_connection_to_actor",
    "network::tests::delayed_superseded_tenure_cannot_replace_or_tombstone_newer_same_source_writer",
    "network::tests::reply_wrapper_exposes_delivery_active_unwritable_no_ownership",
    "network::tests::accepted_draining_connection_delivers_reliable_progress_after_replacement",
    "network::tests::reply_route_tenure_retires_only_after_final_receiver_guard_drops",
    "genesis_bootstrap::tests::unavailable_reply_writer_uses_requester_retransmission_without_parking_old_route",
    "consensus_message_control::tests::private_reader_treats_safe_atomic_replacement_as_retryable_identity_churn",
    "network_relay_tests::certified_merge_sidecar_close_is_limited_but_responder_controls_are_critical",
    "sumeragi::v2::tests::strict_same_round_tc_preserves_and_retags_timeout_vote_owners",
    "sumeragi::v2_core::tests::later_reproposal_commit_qc_replays_and_applies_its_exact_certified_round",
    "sumeragi::v2_core::tests::valid_commit_qc_supersedes_different_subject_prepare_lock_live_and_replay",
    "sumeragi::v2_effects::tests::different_subject_decision_supersedes_protected_lock_and_frees_losing_capacity",
    "sumeragi::v2_effects::tests::apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work",
    "sumeragi::v2_core::tests::height_context_requires_one_same_round_parent_commit_geometry",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_rebinds_lock_and_retains_current_timeout_vote",
    "sumeragi::v2_core::tests::later_reproposal_commit_ack_retires_durable_old_round_commit_pool",
    "sumeragi::v2_core::tests::tc_lock_survives_closed_view_and_commits_after_later_same_subject_reproposal",
    "sumeragi::v2_core::tests::replay_resigns_same_subject_reproposal_fifo_without_relabelling_old_commit",
    "sumeragi::v2_core::refinement::tests::strict_same_round_refinement_kernels_reject_split_round_mutations",
    "sumeragi::v2_core::refinement::tests::wal_retirement_authorization_rejects_split_round_decision_and_receipt",
    "sumeragi::v2_core::refinement::tests::semantic_commit_decision_identity_ignores_only_qc_rounds",
    "sumeragi::v2_core::reducer::source_link_tests::closed_proposal_round_cannot_create_a_new_commit_intent",
    "sumeragi::v2::tests::locked_subject_reproposal_and_strict_higher_prepare_are_safe",
    "sumeragi::v2::tests::successor_context_requires_the_durable_cryptographic_parent",
    "sumeragi::v2::tests::authentication_rejects_valid_commitment_conflicts_without_mutating_adapter",
    "sumeragi::v2_effects::tests::authenticated_genesis_satisfies_manifestless_certified_decision_fetch_locally",
    "sumeragi::v2_effects::tests::reproposal_commit_qc_applies_the_exact_unchanged_body",
    "sumeragi::v2_effects::tests::reproposal_commit_signing_uses_its_same_round_validation_marker",
    "sumeragi::v2_runtime::tests::exact_authenticated_timeout_certificate_from_distinct_sources_coalesces_in_one_runtime_slot",
    "sumeragi::v2_runtime::tests::body_available_rebind_accepts_same_view_higher_generation",
    "sumeragi::v2_transport::tests::reproposal_commit_qc_authenticates_its_exact_same_round_body",
    "block::consensus_v2::finality::tests::header_binding_allows_unchanged_reproposal_but_rejects_earlier_decision_round",
    "block::consensus_v2::tests::height_context_identity_ignores_reproposal_round_and_rejects_split_rounds",
    "sumeragi::v2_core::tests::vote_statement_identity_excludes_only_the_authenticated_signer",
    "sumeragi::v2_core::tests::certificate_height_subject_identity_ignores_round_and_phase_only",
    "sumeragi::v2_core::tests::view_zero_binds_semantic_parent_decision_across_reproposal_rounds",
    "sumeragi::v2_core::tests::earlier_same_body_commit_qc_supersedes_a_later_reproposal_lock",
    "sumeragi::v2::tests::registry_rejects_split_round_vote_and_qc_reference",
    "sumeragi::v2_body_store::tests::rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round",
    "sumeragi::v2_body_store::tests::rotating_leader_reproposal_authenticates_the_immutable_header_leader",
    "sumeragi::v2_effects::tests::deferred_merge_sidecar_accepts_earlier_carrier_and_rejects_future_or_foreign",
    "sumeragi::v2_effects::tests::split_round_commit_signing_is_rejected_before_service_dispatch",
    "sumeragi::v2_runner::tests::exact_locked_body_is_reencoded_at_the_reproposal_round_without_byte_drift",
    "sumeragi::v2_runner::tests::replayed_proposal_sign_reserves_only_the_exact_current_lock_owner",
    "sumeragi::v2_worker::tests::closed_flush_on_delivery_active_unwritable_route_parks_without_cursor_advance",
    "sumeragi::v2_worker::tests::closed_flush_racing_final_receiver_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::unavailable_admission_racing_retirement_is_nonfatal",
    "sumeragi::v2_worker::tests::entered_view_accepts_same_view_higher_generation_supersession",
    "peer::run::tests::peer_task_abort_drains_queued_worker_then_notifies_exact_connection_once",
    "peer::run::tests::peer_task_panic_closes_delivery_producer_and_notifies_exact_connection_once",
    "peer::run::tests::dispatch_worker_join_error_is_returned_after_fail_closed_teardown",
    "network::tests::duplicate_configured_termination_does_not_advance_backoff_or_metrics",
    "sumeragi::v2_lane_work::tests::late_old_sidecar_flush_removes_only_reconnected_source_retry",
    "tests::relay_fairness::hold_release_same_source_reconnect_retires_old_delivery_without_rebinding_new_route",
    "network::tests::deferred_queue_preserves_order_and_connection_bindings",
    "network::tests::flush_deferred_frames_closed_session_restores_remaining_unbound",
    "network::tests::flush_deferred_frames_drops_stale_connection_binding_without_posting",
    "network::tests::flush_deferred_frames_rebinds_reliable_stale_connection",
    "network::tests::flush_deferred_frames_sends_unbound_entries_to_current_session",
    "network::tests::live_session_backpressure_defers_retry_with_current_connection",
    "network::tests::live_session_closed_defers_retry_unbound_and_removes_peer",
    "network::tests::live_session_post_overflow_disconnect_policy_defers_unbound",
    "network::tests::missing_session_retains_unbound_consensus_frame_and_schedules_reconnect",
    "network::tests::rejected_authenticated_connection_is_cancelled_and_remains_cap_accounted",
    "sumeragi::v2_core::tests::recovery_excludes_proposal_intent_superseded_by_same_round_timeout_upgrade",
    "sumeragi::v2_core::tests::recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification",
    "sumeragi::v2_core::tests::replay_accepts_strictly_higher_matching_prepare_qc_proposal",
    "sumeragi::v2_core::tests::replay_resigns_proposal_with_equivalent_parent_reproposal_round",
    "sumeragi::v2_core::tests::same_round_timeout_upgrade_is_exact_local_proposal_justification",
    "sumeragi::v2_core::refinement::tests::decision_ack_retires_competing_owners_and_keeps_one_body_pipeline",
    "sumeragi::v2_core::refinement::tests::lock_and_commit_requires_one_current_vote_and_proposal_round",
    "block::consensus_v2::tests::timeout_proposal_accepts_only_the_selected_prepare_subject",
    "sumeragi::v2_core::tests::future_prepare_qc_is_transactionally_ignored_without_retransmit_ownership",
    "sumeragi::v2_core::tests::tc_omitting_the_local_high_keeps_its_exact_prepare_qc_retransmittable",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_projection_selects_and_fetches_the_exact_post_install_lock",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_without_a_lock_carries_and_fetches_nothing",
    "sumeragi::v2_core::reducer::source_link_tests::enter_view_effect_cannot_substitute_an_equal_reference_certificate",
    "merge_sidecar::tests::full_server_table_never_advances_generation_without_a_changed_roster",
    "sumeragi::v2_lane_work::tests::duplicate_generation_hint_coalesces_alternate_reply_sources",
    "sumeragi::v2_runner::tests::relayed_generation_hint_preserves_reply_route_from_lane_through_worker",
    "sumeragi::v2_worker::tests::generation_hint_requires_exact_reply_route_ownership",
    "network_relay_tests::certified_merge_sidecar_messages_preserve_ingress_reply_route",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_required_serve_gate_precedes_open",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_certified_request_cutoff_blocks_later_churn",
    "sumeragi::authoritative_runtime_gate_tests::fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes",
    "sumeragi::v2_worker::tests::exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
    "sumeragi::v2_worker::tests::repeated_exact_serve_claims_close_all_older_sources_before_later_io",
    "sumeragi::v2_worker::tests::exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission",
    "sumeragi::v2_worker::tests::fair_ingress_exact_ticket_coalesces_and_commits_before_later_io_producers",
    "sumeragi::v2_worker::tests::drained_exact_retransmission_gets_fresh_scheduler_ordinal",
    "sumeragi::v2_worker::tests::fair_ingress_gate_overflow_closes_without_partial_admission",
    "sumeragi::v2_worker::tests::fair_ingress_classifies_current_historical_future_and_unauthenticated_requests",
    "sumeragi::v2_worker::tests::fair_ingress_rollover_retires_ticket_before_old_service_teardown",
    "sumeragi::v2_worker::tests::fair_ingress_producer_episode_wins_or_yields_without_partial_exact_admission",
    "sumeragi::v2_worker::tests::fair_ingress_full_prefix_materializes_exact_serve_before_later_churn",
    "sumeragi::v2_worker::tests::fair_ingress_serve_only_prefix_materializes_after_frozen_completion_ack",
    "sumeragi::v2_worker::tests::fair_ingress_terminal_retry_replays_without_lifecycle_resurrection",
    "sumeragi::v2_worker::tests::fair_ingress_higher_view_waits_out_active_family_before_admission",
    "sumeragi::v2_worker::tests::durable_serve_restart_before_terminal_seal_resumes_same_lifecycle",
    "sumeragi::v2_worker::tests::restored_serve_waiter_advances_shared_runtime_source",
    "sumeragi::v2_worker::tests::durable_serve_abort_before_commit_restarts_into_local_completion",
    "sumeragi::v2_worker::tests::durable_serve_seal_before_completion_post_restores_terminal_replay",
    "sumeragi::v2_worker::tests::durable_serve_seal_survives_post_before_physical_ack",
    "sumeragi::v2_worker::tests::durable_serve_corruption_fails_closed_without_highwater_reset",
    "sumeragi::v2_worker::tests::durable_serve_frame_bound_covers_max_layout_manifest_hashes",
    "sumeragi::v2_worker::tests::durable_higher_view_abort_republishes_displaced_terminal_before_restart",
    "sumeragi::v2_worker::tests::durable_higher_view_admission_crash_locally_completes_successor_union",
    "sumeragi::v2_worker::tests::durable_serve_restore_rejects_capacity_owner_swap_across_replacement",
    "sumeragi::v2_worker::tests::durable_serve_state_is_pruned_only_with_successor_rollover_root",
    "sumeragi::v2_worker::tests::certified_serve_future_slot_blocks_control_and_consensus_replenishment",
    "sumeragi::v2_worker::tests::certified_serve_cross_relay_retry_replays_one_terminal_tombstone",
    "sumeragi::v2_worker::tests::certified_serve_terminal_rejects_mismatched_response_hash_without_releasing_owner",
    "sumeragi::v2_worker::tests::certified_serve_observer_owner_contains_prepare_and_commit_subfamilies",
    "sumeragi::v2_worker::tests::certified_serve_higher_view_abort_restores_terminal_high_watermark",
    "sumeragi::v2_worker::tests::certified_serve_receiver_close_aborts_reserved_replacement_without_orphan",
    "sumeragi::v2_worker::tests::certified_serve_delayed_lower_view_cross_relay_cannot_resurrect",
    "sumeragi::v2_runtime::tests::busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation",
    "sumeragi::v2_worker::tests::invalid_requester_signed_qc_quarantines_one_family_without_consuming_honest_capacity",
)
_PRODUCTION_LIVENESS_NEW_REGRESSIONS = tuple(
    test_name
    for test_name in _PRODUCTION_LIVENESS_NEW_REGRESSIONS
    if test_name not in _PRODUCTION_LIVENESS_RETIRED_REGRESSIONS
) + _PRODUCTION_LIVENESS_POSTCUT_REGRESSIONS

# The retained executor queue is the concrete FIFO consumer for reducer
# batches. Bind the complete reviewed methods, then additionally check the
# semantic ordering fragments below so failures identify the violated seam.
_PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256 = {
    "consume_effects": (
        "e52cf5a95fff03c6bf9982ac6f8933cfd940d0e8c4d2673a22a73a17ed5f74fa"
    ),
    "drain_retained_effect_batch": (
        "d973c4b573c75870a27334050546a1d9f7996ca8f5da6b0c4fb96d23c985e39b"
    ),
    "step": "7f88caed1111bc8a98699d31100deaa1fa9d3ae01b9891b4396acd546f17d3d7",
    "step_pending_tip_recovery": (
        "e5a7cf7d1db1e5558d888b22275a578baa273dbcbc6e282047e6c8d399127172"
    ),
}

_PRODUCTION_EFFECT_SCHEDULER_HANDOFF_ITEM_SHA256 = {
    "take_scheduler_ownership": (
        "bccdd7c65959f3921d46373c86d1d6545568a326ddea42379362c7b1970397cf"
    ),
}

# Exact comment/literal-free token digests for the production exact-output
# scheduler and applied-height retirement seam.  The scheduler digests bind
# target-local/global FIFO ownership, round-robin service, and exact returned
# post retention.  The retirement digests bind the narrow Retransmit whitelist
# and the Kura receipt/finality-artifact authority checked before output is
# handed to durable reconstruction.
_PRODUCTION_EXACT_OUTPUT_ITEM_SHA256 = {
    "take_attempt": "acc18d3997a0cc6fcca4926b72a63fedf5d0987ecb33c1114e93e0da3b2254d7",
    "mark_admitted": "c6e502433ef5249540446d75e0f88f665a7ffc456bf4014216f808fd123f072c",
    "retain_returned": "8f1436db10edaa22360b416024817e8da58752a2fa7b604d97b0f6819976b0cd",
    "owns_source": "6a89444dd116f019fabd7ef2465b5ccc9d0b404adbf783dc47d47ad150ebe1b2",
    "target_is_local_head": "a66a280165f7945efa150447183cb391824878b3d002c7cd7d964f60d8a44096",
    "advance_target_cursor": "b16e6854e6b213b04e89a740f8b20f1c00f13ae88e8da38a39faac55ec6b0c26",
    "handoff_applied_height_to_durable_reconstruction": (
        "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80"
    ),
    "target_is_global_head": (
        "f71465dbc379cc235d6669a17c7cc6eca1b456acbf5b8b996ab39ebebbac05ee"
    ),
    "next_schedulable_target": (
        "46a5fc46be8b95eb70516a03c7fc52537238a7fe26545540a1a85413e3465475"
    ),
    "advance_after_attempt": (
        "e678eb75bf1e124b8c3ac7b196bc9abee8d5429a6f064a1aaf64851291bc0e07"
    ),
    "drive_with_budget_ack": (
        "a4453951bf9775ad83b603cfe9b5f5849cc6251904602cbf76438c6140703bfa"
    ),
    "drive_bounded_with_ack": (
        "7e630c6be2466fe0980df81ea194e101b70e4abac9546a379ab9f24c8eb23861"
    ),
    "applied_height_reconstruction_covers": (
        "acd0c53df363cdc5301f6619ec69ac0a37bd1ae83ff2d337303574ce2510f4b2"
    ),
    "durable_exact_output_handoff_owner_pair": (
        "f848a53734e173e64122e3cf198d6e0adfea99909594e667c56a7e11ea271a84"
    ),
    "certified_sidecar_prefix_covers_occurrence": (
        "019ae94e6c1ebdea5e6948361cf117d49003d4433cd9daa7c3a28f2e1e391741"
    ),
    "handoff_applied_height_output_to_durable_reconstruction": (
        "2064071eca55b70e01a51bda015e5cda8561a35d2f0d9e09559ac23c01ae6f8d"
    ),
}

# Complete token seals for the source-isolated exact-output corridor.  These
# complement the projection checks below: a mutation cannot preserve one
# positive subsequence while inserting an early return, dropping a sibling
# source, or disconnecting a writer-flush receipt elsewhere in the same item.
_PRODUCTION_MERGE_SIDECAR_SEAM_ITEM_SHA256 = {
    "CertifiedMergeSidecarChunkAdmission::from_admitted_reply": "a2b593a56511c97eae5bfaca2235490ace31a128884729704bb6dc907a98da44",
    "apply_reliable_flush_application": "93912d6c690daa60be7fda65daa0a997a0e55839d3661560ada00aaca8dd8a52",
    "MergeSidecarTransport::with_limits_and_server_stream_capacity": "acdb21ade067f5460c4019cd4fb0f65741d2f2707ccd1ac447d260641dc3b7f1",
    "MergeSidecarTransport::derive_server_request_capacities": "f8b22bf3ac205e2b2743922978abf18539f87b885cab62874e7c86996a3d9870",
    "MergeSidecarTransport::begin_request_or_close": "8fae96ccb47e45e090ba0ddad6359153951f47583e23be35c6eccfa6ce61e3ca",
    "MergeSidecarTransport::release_unsent_request": "dab1c81134717c57640cd64da49a68ab99cf61d474e02d7b4f1c0ee0b398793a",
    "MergeSidecarTransport::release_authorized_server_request_attempts": "713f97a0a252983fffcd258d631f1052cd3801f634d8554d1febcdc65cf689d9",
    "MergeSidecarTransport::park_authorized_server_request_attempts": "a7f6ad8f3a595fe655f0535b11d2564a2a783d60df1cc6bebeadb704606e96f6",
    "MergeSidecarTransport::reclaim_inactive_outbound_attempts": "9600b575eb21edcb26955867d706880f5040c062ee84b45c266efeb5d484a1a7",
    "MergeSidecarTransport::prune_server_gates": "13f9e547c581b576aed5a552fcd2ecd2f0ea7260fe5a72c7bd7a78832ceb61c2",
    "MergeSidecarTransport::server_request_source": "c1c1f62439ac7e654a67db7cd7de13504315c3da463da0ac7d639e8c3722bffd",
    "MergeSidecarTransport::source_gate_count": "61c262742b3758298046503a720335804b1981748c8bc156527dce9705c80bc8",
    "MergeSidecarTransport::server_gate_attempt_count": "e9c80512a68ae8368554aa4e6990a625ca57a5b6c5fb47ecb5b7bf1598a38acb",
    "MergeSidecarTransport::outbound_attempt_count": "d0b5ac92c19eab61d75f4b461c6ecb4a64d6861fd5baf194a52737bf4843879f",
    "MergeSidecarTransport::source_outbound_count": "3b17f1f9955d281381cdf58b38f910687a62423dc48f8ddfa96fc150d401f451",
    "MergeSidecarTransport::global_outbound_bytes": "1ec3ffa56334677b60c9c920e8e15a562ed04653674849e665b2d44c623c73a6",
    "MergeSidecarTransport::source_outbound_bytes": "2c4678e27b465dd69d9b36e07d9f8b1e85b56f2568282a90f9bbef47a919c3c7",
    "MergeSidecarTransport::route_update": "2f8d4bf918efcb9070aedbb7e4d57baa89f8127ebcf4375c53af35afa852856a",
    "MergeSidecarTransport::alternate_source_is_authorized": "8ccfab565eb86d3527950b783f0ef326303e0a386dc10f268e25f72fcee25421",
    "MergeSidecarTransport::route_source_capacity": "162b152d56446bffa077c6005f4457ce2757a26a73306018a80cc53f68a9ba25",
    "MergeSidecarTransport::can_add_outbound_attempt": "7ec9222597fcb0ada3b06a589d775066cd94335c8d6bb2cfcb2f685fb922c43d",
    "MergeSidecarTransport::next_server_request_materialization": "445d03272e85e2bf3f7e3f054c2acfa2d8ff6434b86c62defca965bc42bc3002",
    "MergeSidecarTransport::admission_after_fair_materialization_selection": "26e50ac363ea2e8c75497ad2df045ab1dcd4ed2315db16147cd9488be04d82e9",
    "MergeSidecarTransport::admit_server_request": "78fcf00b793e0ef8b871657c83c3b05b8b81149e789470faca831fd8c3b50fc8",
    "MergeSidecarTransport::cancel_unmaterialized_server_request": "40f0831462c1e44e4e55919d9ba9dfc443b6901dd963c47ad79106ce274586fd",
    "MergeSidecarTransport::enqueue_response": "2e11a16de83ba8372b3ad92e58e6b222f9268d70611be2bb1c56f765f4aaa06b",
    "MergeSidecarTransport::drain_outbound_chunks_durable": "b15f79eea3fa0a065bed3026d821e05a7562e379e24dff34db19c8b32b601ecd",
    "MergeSidecarTransport::acknowledge_outbound_chunk": "9f9a801118e363d0d821ab528855b60354a0eeaf3acd186191d7da4512155484",
    "MergeSidecarTransport::tick_bounded": "8a01af0d92b9474c1b9c4ff0c861d2b29315b4681062e1db668a4b1db543c541",
}

# Complete seals for the crash-safe semantic request lifecycle. Process-local
# reply capabilities are deliberately absent: only stable peer ownership,
# sequence floors, terminal/pending cursors, and immutable pending-chunk
# identities may survive a restart.
_PRODUCTION_MERGE_SIDECAR_LIFECYCLE_ITEM_SHA256 = {
    "RequestStreamState::allocate": "035e187a842019c9adf402f2984a0e69ea7de331056e6038d0f3974311e566ee",
    "RequestStreamState::close": "ea56cf4ad45b41875017f40148321ce2189dc77919fd6dbd60ba83b45107166f",
    "RequestStreamState::emit_close": "662b1d15ddcaa2ba9b0ef4c776e7d30647f5aab383ffddeb5bf6dd567178a552",
    "RequestStreamState::acknowledge_close": "4a2e627dd28de3c332a45c68a351d86bb392ebf07a9c4a4fa8021399bfb78dd3",
    "ServerRequestSource::budget_source": "ff85c7db75fcbff14abacdbd2d23351e5c244b8046989f7ed6d8feb2bfe59663",
    "ServerRequestSource::shares_budget_with": "d2fccc4fab299dac39f9a6bac5639887a4dd6e115eae29ec90579da93c75051e",
    "CertifiedMergeSidecarGenerationHintV1::canonical_hint_id": "2731d88621ae21e7119162b150578b5419f3b1a7e04676f9565e6d2f78a0ebe7",
    "MergeSidecarLifecycleSnapshotV3::new": "b76fc1b3f9d52cd47e221b55ca35cd814bf86290cac86b7de8c8150dff0d8754",
    "MergeSidecarLifecycleSnapshotV3::integrity_is_valid": "413dd7181f9898e41bb9b9f2bb826ceaa793c44f9a4a12943d36fe58ae32cf0c",
    "MergeSidecarLifecycleRootHighWaterV3::bootstrap": "09c144912ad447177997f12fc31de749830f39e500a726e7ca7d0490ccf0de51",
    "MergeSidecarLifecycleRootHighWaterV3::new": "6fb62886f1fc7515ccb93708a257239f55e2fcb252543fc170427cd2ed4944fe",
    "MergeSidecarLifecycleRootHighWaterV3::is_bootstrap": "af4f8d7064a8e7d4334a2fd28b6979ce8f97040f9a8fcdf79840e823b6e41621",
    "MergeSidecarLifecycleRootHighWaterV3::matches": "3138b2e1831a9710b7dcd84570fdeceaef945398cb0bb1e16ba7ff585a431d1f",
    "MergeSidecarLifecycleJournal::open": "81f2b35d6df08c5cac28ebb95fb240a7e5a949e52283c7f4ea5ccd8a357fdd33",
    "MergeSidecarLifecycleJournal::state_path": "e1a8db899e4e4ae845a2a5d5c7731c3ec7a4d04a57494a90876c761063b14789",
    "MergeSidecarLifecycleJournal::state_path_for_generation": "958b5b38f871dc10d788964dd806276f7112376c426412c402ff66d2086acf48",
    "MergeSidecarLifecycleJournal::temp_path": "4023b2cff04791329db79483fe6ac8dd2af85f90ded291d23c7cbbb33c17bf7a",
    "MergeSidecarLifecycleJournal::root_high_water_path": "fc93b03a37534d588c93fa488d50dad2909c133463a2d11cf0176bf42c8e05ad",
    "MergeSidecarLifecycleJournal::root_high_water_temp_path": "775a520e25b9c83c46aa08944341590719b4d59d0d2d12af95fb9d2c968099d8",
    "MergeSidecarLifecycleJournal::publish_bootstrap_marker": "40c9b879e62f60d03b3a81db96b1c67a9f97fefb36fe5453e7cc44ecdf836c92",
    "MergeSidecarLifecycleJournal::bootstrap_candidate": "5b0488a94395189e31943072b55da8dc71da1dda18106afeced87f73ed7c1048",
    "MergeSidecarLifecycleJournal::finalize_validated_open": "86894b3b15406d99916d483471c8ebb16ca3234745e93335768d100fd0bd1b0c",
    "MergeSidecarLifecycleJournal::sync_directory": "6bebdd14196431dd0c8c9160bca5965b8c902ae8fd9addd79596e6ff65ca9d8b",
    "MergeSidecarLifecycleJournal::artifact_exists": "f975c5a57d4c64323fa39d6fc2439658d13d3db6b944ad62805feadd841f7edb",
    "MergeSidecarLifecycleJournal::reject_artifact_if_present": "0d3908089c208e1abb49f232c1191b354960c100f1b3ce7dafceda699adfd7f6",
    "MergeSidecarLifecycleJournal::reject_known_temps": "3e359c0b5dc40cbb9e51b962ba8c840c9dd9837bca38378b58d38c95346ee06b",
    "MergeSidecarLifecycleJournal::remove_regular_artifact": "91167e9513b22b8b66d14f6fa3891510e4dcf88964f1ddaad777e6c69d76b911",
    "MergeSidecarLifecycleJournal::validate_regular_artifact_if_present": "4ab43f67d2bc6776e74b901e5cae02dd27c4259e4b5e525d86f71832893d2528",
    "MergeSidecarLifecycleJournal::validate_known_temps": "79f498aac06957b42a448cb12169f0e88612792854cf870e9065fc1501b14044",
    "MergeSidecarLifecycleJournal::discard_uncommitted_temps": "5075863b4359aff4aca8f37895fa497cfbcf82298ddef44e5d439a92e8a337f9",
    "MergeSidecarLifecycleJournal::discard_inactive_slot": "675c63c3195613a66fd9551590ebedd83e2f58527f13a0ceb04aab1f757926e9",
    "MergeSidecarLifecycleJournal::validate_directory_entries": "2c13ac3a3cec9f55d88eecf2f7781635f70fd70c824f71750e35332f6b9d2cec",
    "MergeSidecarLifecycleJournal::read_bounded_regular": "38f7bc44bed7b7c1f1d9847e75f65430dc6bd15532827ce888c0c8ba824e0563",
    "MergeSidecarLifecycleJournal::decode_snapshot": "2473d8b7f9040a2a041bbc46f9c23a8f8d1940ebc779a455edf63590f5a1c402",
    "MergeSidecarLifecycleJournal::decode_root_high_water": "76f8a0caa1f65018f82353e36d2bb07846cb8a49583aac60483dd7eab0c818c7",
    "MergeSidecarLifecycleJournal::load_pair_strict": "c3e273a90e4d180f08476d9bf7dd21b2ce91f0032b227857c09fbbc9b119cc5d",
    "MergeSidecarLifecycleJournal::load": "d7c2d8180e6579b1b8213fddf5ffcb984ef7025a52e66fa5ed4bc2d26c6bf6c7",
    "MergeSidecarLifecycleJournal::write_new_synced": "ab267b2611abb24926fad0ad306da0cb44a49eec565cd7450c76f762473fb2fa",
    "MergeSidecarLifecycleJournal::persist_atomic_replacement": "e8a844216e2110bf463f9884e7bff9d06b8dd0a9d6baff8cf1835bbb29798064",
    "MergeSidecarLifecycleJournal::live_generation": "8d2d97c739d2a32c7e899751791a8ef9528bcd6a5a1900a0f0f7e017a27a2fde",
    "MergeSidecarLifecycleJournal::preflight_next_commit": "5c58aa339bb87024e46da01db648c75a7da2e3e78f12fa7a285414ba70e103f0",
    "MergeSidecarLifecycleJournal::persist_next": "e4b9c82c810f07d1158140e054ffc8e3f07ed143b4e0d68f17e0f3d136c541e6",
    "MergeSidecarTransport::lifecycle_runtime_geometry_v3": "34f7fde0aec9a4d933b9bfd71afa80c9391911797b2db30074f01db8110d119f",
    "MergeSidecarTransport::lifecycle_geometry": "eae65fb2d75f710c039fec57b4b4cfa0801b6620393a1ce90958c6656cd6496b",
    "MergeSidecarTransport::lifecycle_geometry_for_server_roster": "4b6d81251a26903563d0d5a7b309e2b91ebc16fde3e9d78cbae72953d9694d45",
    "MergeSidecarTransport::lifecycle_max_snapshot_bytes_for_attempt_capacity": "1687861693eb80d7b15ee5dc86a8f48227e45a20555318e70c29107c4010e1e3",
    "MergeSidecarTransport::lifecycle_protocol_max_snapshot_bytes": "29795424b1083bf3c351ab3f262b0ee84f13ac0dc903b22767aa0449590ec8e6",
    "MergeSidecarTransport::lifecycle_snapshot": "e51da54b26a259562e37b630f1a3c5e6f6ee6c3dffff2824557fc16801f4e6b5",
    "MergeSidecarTransport::restore_lifecycle_snapshot": "8627186b8d588686e3cb570af7f57cc72ee6781910adfb59c0c5de37b41d47b2",
    "MergeSidecarTransport::configure_prior_lifecycle_server_geometry": "ac39616a1ae23408941413d2472553d6234589168219cf31c0a47b7aed7dcfcd",
    "MergeSidecarTransport::open_durable_with_server_stream_capacity": "3e2ed1cb57775d36918a553dc3e17e065a4e994c87d1e67ddfef2ab3b8344a0a",
    "MergeSidecarTransport::persist_lifecycle_projection": "721b44da8a5911f197ad7326d8cfcb7919c8df7bbd1ee4413af1817f5b03eb11",
    "MergeSidecarTransport::preflight_lifecycle_mutation": "e792b37472d616f160d5dd5d2c28f869d35670df261e31ede7e56d8b93b55030",
    "MergeSidecarTransport::persist_lifecycle_state": "4a8658892bd135d9065f5f8aee9ad6beb73f7f3993b54dc3cb1e7c236bd5fd2b",
    "MergeSidecarTransport::rehydrate_with_exact_geometry": "95238c6a27265eaf9030443a5c92afdbbbe071ebf1337684f3b8f4b88b25aea2",
    "MergeSidecarTransport::rehydrate_with_exact_geometry_after_durable_handoff": "d5c8d8cc61fbe16f2ad8b91595ed63f6ca477e0c8b5269be6417ad486472e821",
    "MergeSidecarTransport::rehydrate_after_lifecycle_restore": "2bde0f988a4a6eacaebdb73d0c19bbd3153320663f64259cbb7e017f78d9e23a",
    "MergeSidecarTransport::validate_retained_height_geometry": "a52a9db9ba2745b1555ff5422edc3c64fcbb3b910ae5f62216009866b706b923",
    "MergeSidecarTransport::requeue_retained_outbound_after_height_rollover": "a09ba68802f834de1a13219085ee6f24e7d1bca666a4c951b80501489eed0be8",
    "MergeSidecarTransport::allocate_request_sequence": "4a00c1f454545b38973384a4a4e968d8eb725f80b9d5c9b54b03afed0327c010",
    "MergeSidecarTransport::close_request_sequence": "d13ef957882ac705a2fdb715f72faf705c2d6c33248b354b61b834f82ce5ffb5",
    "MergeSidecarTransport::begin_close": "023c75991bc11e8528e4692fb11101e1e565f18f496be829eb20a4935f13a9f0",
    "MergeSidecarTransport::begin_request": "1e337d236787c31def6238bef35a2e461abcc78e70b47a54347cb7948e29195b",
    "CertifiedMergeSidecarClosedPrefix::covers": "21db3c4644755e3724626fc5c862b41346facb61576c135749144bd17ba803a6",
    "MergeSidecarTransport::acknowledge_close": "a6dc61f720126bf504dba5bc556385739819b18d6d9af845034b7e175d1ad0c5",
    "MergeSidecarTransport::acknowledge_generation_hint": "86b52bcc4c6063ea5ddf2ce983bd204784d5c129843e5af9f9db50c1d3cd4efd",
    "MergeSidecarTransport::generation_hint_post": "8f16175266e8ef580a910ddb39208d1963c471d058f164222428a90bf78eb922",
    "MergeSidecarTransport::preflight_server_request_stream": "039e4f6bf2d7ed58244185b7a5811f40b19737e953a603eed417f960546008c0",
    "MergeSidecarTransport::record_server_closure": "a8027bf18c82e939aed4072172cfe52b3c3116d5325a3f08c0b611ef95ffe6d9",
    "MergeSidecarTransport::server_generation_is_terminal": "91daaff340270d5a630e818719ce26a4b02d6264e80c4d4f2114bb2eea378f38",
    "MergeSidecarTransport::transition_server_service_generation": "442b14db6826182fa67c8563a3dd7412e7fd2bcc8824b8575640512547665675",
    "MergeSidecarTransport::transition_server_service_generation_after_durable_handoff": "7bf098b2a24c176a5bdfeb473f860986a3219675d10428be7550af1fa249a797",
    "MergeSidecarTransport::transition_server_service_generation_after_exact_output_fence": "68251fab01ae053907984e39eeac0779f1018d50cc069e791a8a85fe010e369a",
    "MergeSidecarTransport::prepare_server_service_generation_transition": "ae7d51586773145d9933369af877e09257c66def9ba4300d4c251109cdc0dabb",
    "MergeSidecarTransport::commit_server_service_generation_transition": "bafcbce053766a2f838c437d8c4f55cbbae22fe22f91e0e5a94944a707146684",
    "MergeSidecarTransport::ensure_server_stream_slot": "54412c913251408bb8da2595fb3fc36ef3b7df4486f09e8c85f8ab821f887597",
    "MergeSidecarTransport::supersede_server_stream": "4bfce430840bb6fd125779e211580bf98b60358736e735d2d124e8726d8a8bc1",
    "MergeSidecarTransport::advance_server_close_floor": "a8b0fe4fd6c15a17c344f1a318e39421efffaf3c24693e1965444d7c4ccb78b9",
    "MergeSidecarTransport::admit_server_close": "b4f0563eda7a12f58f3715b84f68ded706d03274c1744756ce8ba56a3cb2f073",
    "MergeSidecarTransport::drain_closed_server_prefixes": "517b76f57cca2c96ed8d1fbd53db8318b4003e09c7401dba9c8478ca7217f1c0",
    "MergeSidecarTransport::confirm_closed_server_prefix_handoff": "76c4d49654aebfb34456efc2fe5f0ac1737226a954bf3916bffe2ca5319ad7b7",
    "MergeSidecarTransport::server_gate_attempt_count_after_close": "d10ba051868323f576c5030b9d9c73c4c8debe4ae7bb0da21a04db9864e449f1",
    "MergeSidecarTransport::server_gate_count_after_close": "7b3476b7ec3e31013a84f9c331795d6c1218d03ad614734c2e17003e554baa0a",
    "MergeSidecarTransport::source_gate_count_after_close": "a065e18f460113a9ca617675037064f607d592e4edd456744a5ea8a2bcc97a24",
    "MergeSidecarTransport::drain_outbound_chunks_inner": "999e7704f60196377c7e7ca0c1f380b0e774d889ddf4ae34b2bfa084b96fcfee",
    "MergeSidecarTransport::finish_completed": "f3bfa218ed218197e1450d68ca7538c3dbc45c1ea5e2819035c49baaff3b3d62",
    "MergeSidecarTransport::discard_invalid": "d63d81f340e4309f0b795a5901f66e71ee0cb315910980012db0ec0f91b4e254",
    "MergeSidecarTransport::retain_pending_blocks": "498f9767b397cd0466e98d3fe8801d6a8de1d319169c19620a023a9e785e1117",
}

# Attribute-inclusive seals for every platform branch which establishes the
# directory/file identity used by the crash-safe lifecycle journal.  These
# helpers are part of the trust boundary: validating a path and then reopening
# it without `NOFOLLOW` or without a stable handle/path identity comparison
# would reintroduce a TOCTOU decode path.
_PRODUCTION_MERGE_SIDECAR_ARTIFACT_HELPER_SHA256 = {
    "lifecycle_artifact_identity": (
        "301e701d0753691df10a350b69a5eea47cdde38195e156c8b6dbaa5e72aa0d61",
        "9165ef4e4c97ef04d2ae55b5b00605853a2db11688432cbed96a61f567f3a86a",
        "ee13fb0e994aedad25ddab6ead76f0cf992219ab68beffb348f51b1c532bddaa",
    ),
    "lifecycle_artifact_revision": (
        "119f0656e2127e4c66bc6915ad8df49d2710006aad9744122cb17ead34b51e42",
        "9bea5709b4c9f9fb0f06d2003e3a26f696af5bf8cfc51b590a842e113cd0956c",
        "f3123c09a7b193028dbafb465fe169b2bf55d1e00fdba6f185a28d5c33b3b097",
    ),
    "lifecycle_artifact_identity_available": (
        "6b621ac96e7d735fdf061ca29f91228fa68e94eeeeb07f5a8205f56e7f48b225",
        "c21870b387d5cd786c93f51f3873466a002cd6dc9d532f3c950e7b53e44d4bdc",
        "f8d258b48fdda51575891c262d03635f0d8026f500740e5c03a24e67012e35d8",
    ),
    "lifecycle_artifact_is_single_link": (
        "91a822912011db14fdf25c7d86a33d69e72e2e25daf250d3513a3bc1252a0b82",
    ),
    "lifecycle_artifact_is_reparse_point": (
        "e0f9e851429da4dfe4f5b2691083c78fbf53272129be634fe06ddd64cbaf9514",
        "f3613fe601a454d7c1a960b49b86565671cec04b4c6dff410272ed37b722c2c5",
    ),
    "lifecycle_artifact_metadata_unchanged": (
        "2ea5f87b8ceb44ae3ef92320ec86c3c6b88b66d49bbb1dffc9574077189641db",
    ),
    "verify_open_lifecycle_directory": (
        "98f56261beb3cd37739864c2f0223e479177000fa6581bfde5cc85e2418e9da1",
    ),
    "open_lifecycle_directory": (
        "1c480a6f24d1d39a6fe9c7c37bb9cfc738cad69609082b66c60531f141f3fdec",
        "cfc8db5f4cd9eaee8a95a476440bb53c2f39423b12860d444ffd713c051f1ceb",
    ),
    "verify_open_lifecycle_regular": (
        "b3354225cd5d701857ffc3ddd06b958ceffeb4b3f611d5bb7af9980b7f52d12f",
    ),
    "open_lifecycle_regular": (
        "1762c07b05b671e00fa3b476b61267510a4313aac498d6f440f4b29651e3836e",
        "536e5ed345f17fe72ce9856541070e24c9cfad92ae8da06e8ef18c4024c54c2e",
    ),
}

_PRODUCTION_MERGE_SIDECAR_BOUNDARY_TEST_SHA256 = {
    "authenticated_source_quota_rejects_origin_churn_and_preserves_other_source": (
        "177b9dcad1c91ebd413bc7824e2a2340c20287fa89d1899f83df154146394233"
    ),
    "legacy_lifecycle_v1_snapshot_is_rejected_without_migration": (
        "c100cca083e1def146023e86038c60b04ae9a641a2e29dad0a7f2926e7232e83"
    ),
    "durable_responder_restart_preserves_same_hub_gate_budget": (
        "2c98602d0876f99438260e9accd99459d09334d014e6707e497cbdf7bdbe3ca6"
    ),
    "fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses": (
        "72591d58cb56aa08d3ea0bce920f7cf36c9a00f6b14593bc5dbf17f3834da444"
    ),
    "quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps": (
        "e5e5d27f264d8c30229eb80df95e7c95329c5fb42eb3370f0b3a4ddb1bd37342"
    ),
    "durable_lifecycle_v3_root_high_water_is_exact_monotonic_and_noop_stable": (
        "34edbc3a0e29cb88e9c672095506bd8e40aaeedeaf0faa63991baee4aa33c4d1"
    ),
    "durable_lifecycle_v3_bootstrap_recovers_first_commit_and_rejects_missing_roots": (
        "e01fd4052a5a78e492624de151d297dfaf91f44f47894a7645f355e253bf3f48"
    ),
    "durable_lifecycle_v3_rejects_crossed_bootstrap_and_committed_root_shapes": (
        "6288640952d9ea0e72797895aa2de7dd62186daf81da33497bd4bd5a89518c83"
    ),
    "durable_lifecycle_v3_recovers_regular_temps_and_rejects_unsafe_artifacts": (
        "cd06d9140fd9f052787048d9719feba1255e670d4eb0fa619aee24ced110be00"
    ),
    "durable_lifecycle_v3_validates_semantics_before_retiring_crash_artifacts": (
        "b9802b3eb8ebb243b566f6b8fad320369912c600111d3c045f36b9f1674cfd49"
    ),
    "durable_lifecycle_v3_rejects_split_generations_and_rehashed_state": (
        "bee7172a2a75e10ae2d34e2da9ebc8f688bf152cb9c74de324cd38bf9cf0ef10"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_close_mutation": (
        "529af4e365c92c84a976a06f6a1271bb41afc5878a4206bab39bfef06726b908"
    ),
    "durable_lifecycle_v3_generation_exhaustion_precedes_writer_flush_cas": (
        "22bd6d8a616406275daf917c063d1af71d5196ee9953320f4c6615873f8e85d8"
    ),
    "durable_lifecycle_v3_recovers_predecessor_before_state_directory_sync": (
        "6c51610572f862a27b7c4cdb704fd85b9cada24ea849313a61078091782fb518"
    ),
    "durable_lifecycle_v3_recovers_predecessor_between_state_and_root_publication": (
        "37fdf12044f2511e5125de87e8d468d61831be1328ca1778b4737e0518b38920"
    ),
    "durable_lifecycle_v3_resyncs_replaced_root_before_predecessor_cleanup": (
        "d8ac3e60c1d433064f25b8d75226080e3cfe98b0f955de14d29ec769505a631b"
    ),
    "durable_lifecycle_v3_recovers_successor_after_root_publication": (
        "7664893351c81f1a06df874c5a133539e6aab8f8d5ea95158419e2f8c8caa7c1"
    ),
    "durable_lifecycle_v3_rejects_missing_state_with_surviving_root_high_water": (
        "7da31a3cbc8c7d50c7cbb5015a1691b73827385de9598e4fa8cc5db8a53c7295"
    ),
}

_PRODUCTION_LANE_ACK_SEAM_ITEM_SHA256 = {
    "V2LaneWorkLimits::new": "5224fe67da91d648fef4cb803ffcd48e972fbd3b32dc85917773666adf39235e",
    "RetainedMergeSidecars::rehydrate_for_successor": "709b44e4cf845ffe76903ad4d7f61b9fa174ccc1f0a1a793d6733a37a23cd0a6",
    "V2LaneWorkAdapter::new_with_output_guard_and_transport": "edbb3508e583943428c030cbd22188183f683c9bab025f3d30f1b6f9f19d9fa7",
    "V2LaneWorkAdapter::into_retained_merge_sidecars": "96d1e194eda3660ccccf9b4e1860b26a4db8d9ee9fb2bad4f988e37c85627218",
    "V2LaneWorkAdapter::accept_relay_message": "8c307167fe5ea486fc509817d12c352fa8a831751e0e7fc1342b6d43b6ef00bf",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar": "8e348b518da26f91fff8840d1e5bf4016d783fef4720bc275baba90ebe14a0bd",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_request": "308ed2709d25b282cb080d8c74cf781a37b84e28c3160536405a3587c749d16b",
    "V2LaneWorkAdapter::accept_certified_merge_sidecar_close": "6ed196ef77d4dfa0a97eba4196df343e16bfd9867550cdf2689a127bc555ffcb",
    "V2LaneWorkAdapter::apply_closed_server_prefixes": "ca526c1cdcf5ade6263ec12641207c4edf83b5ca71c48a5ed39c8bcdc972a2db",
    "V2LaneWorkAdapter::coalesce_closed_sidecar_prefix": "70aeb1f315515d6c69f913afed4e4874b834d74972c68d1058a8b8916b03e771",
    "V2LaneWorkAdapter::drain_closed_sidecar_prefixes": "2d0a07530ab64b95f687a48fa09cd029a40bad9c18ef29bf82e8b624274bf2e3",
    "V2LaneWorkAdapter::requeue_closed_sidecar_prefixes": "97bdd3f66d265ba00e0cc088af59f005e7cfdc97c138cad9faa64138f944df2d",
    "V2LaneWorkAdapter::confirm_closed_sidecar_prefix_handoff": "7aaca416f7ad687d7ee0c1b99da038d11dbdc710d81229c340a63bda10476c28",
    "V2LaneWorkAdapter::stranded_retryable_sidecar_control_index": "09ed26efa19aefc39d448b1bee81d5b070c272557137e89a51c4f1dc6334419b",
    "V2LaneWorkAdapter::replace_stranded_retryable_sidecar_control": "13094aa7fc37a32648ebf74c461802e857173858f09d7c61ef0054530e340e4c",
    "V2LaneWorkAdapter::service_next_certified_merge_sidecar_materialization": "8e684a41c6ed9799d6fdf7598d7accf8f175f38ff1388058568a6f8bc077c9ea",
    "V2LaneWorkAdapter::persist_anchored_sessions": "40f2ca1c93171337adae168019d2f28ca65acf838da2e9011f6e751ad775876f",
    "V2LaneWorkAdapter::hydrate_canonical_lane_artifacts": "6ea4db2fb530ea0ee56548f72c82a6df6370e3169ff1930e7a748f85d1ca3242",
    "V2LaneWorkAdapter::next_effect": "62af9ea4c3707845b5b097a27f5cc9281b8ade4bc60db49cdbc9f1c3e2b3496a",
    "V2LaneWorkAdapter::effect_count": "3be06e0c96fdc63e06952ec83b5aa900daf39912955249ca6aad64ec50e1354a",
    "V2LaneWorkAdapter::requeue_effect": "5259377bba158615135666cb3cddf88e0fbfbdb63e55a7691ba397e34195d856",
    "V2LaneWorkAdapter::drain_effects": "478982ec7c7cec9990a70993011e34e0cf79f57fb903b3c7cbabc040052b1aba",
    "V2LaneWorkAdapter::push_effect": "cc9f8f5b9469904d0ba6584db2ef13b8aa9ce2f6ed941b5b34e8b7eec3e7717e",
    "V2LaneWorkAdapter::schedule_retransmission": "7468d25a90d61258242527880622e74ff38143c0f75c2e7bf572c9792c9f6232",
    "V2LaneWorkAdapter::schedule_retransmission_at": "541e8582db809b0bb449610cbf70d61f4dafb806b22add9642baabb557eeb994",
    "V2LaneWorkAdapter::prune_finalized_merge_sidecars": "b8400dca9234242c7f6b8583ffabed34eb17fecc23cf5fd81799bec5cd692af7",
    "V2LaneWorkAdapter::sidecar_effect_slots": "13a99e0350b8d59489cc591573adcc2b1f3f775308e9b225f1672649235699e6",
    "V2LaneWorkAdapter::next_sidecar_effect_selection": "614006f927ec384764396c8742a3b05b5a70907dddd8bcf39ed2db0aba4d8975",
    "V2LaneWorkAdapter::push_merge_sidecar_post": "89e2c9f8586ac17ba40acbcf14b133193ff0aade68ce717ec24c2820dce07301",
    "V2LaneWorkAdapter::push_merge_sidecar_post_or_restart": "85ca2f652d8409223ea2fd331d3c9010710301b5aafc0e2082e2b63557eed2e9",
    "V2LaneWorkAdapter::remove_acknowledged_sidecar_retry_effect": "14002f78eb6eee073c72b1c5fa547c69187392e1a2085d8bd5c1e385fbbf2efb",
    "V2LaneWorkAdapter::acknowledge_certified_merge_sidecar_chunk_admission": "60809702ee6e6fca12c654f9e93a453e9a4098be03608ccb11a6513cc5a6b5c5",
    "V2LaneWorkAdapter::push_merge_sidecar_effect": "08a2b77d14faf80060a2ca76a491df474dbbf548b3f0f4a5f95c13d14102c750",
    "retryable_sidecar_server_control_has_writable_route": "4f7ac1895057d195e13c094178010703e09198a4b6824a96b75739072c1362eb",
}

_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256 = {
    "run_inner": "3d8911e69d491268dabae08f5c6b996ede417dd8afbda670e80dfc1070ed2271",
    "require_peeked_lane_work_effect": "bb5763cb4c16586460c17c92f9578a5431c976fb83bc512e94e84646d6e5c1da",
    "lane_work_limits": "320507830881ae53c67850d75b030dcdddab32c0ccf2814f8d6bd6705fced09e",
    "apply_bounded_sidecar_admissions": "27eb4ede4dd038babb38255b89f6a25259b79f55c6dcee33779efbc5d91e04ad",
    "apply_certified_merge_sidecar_chunk_admissions": "0243d1f22247947cc44ac474293a9c852c63509fd46f9357e4ce56b3fd0be518",
    "apply_certified_merge_sidecar_closed_prefixes": "4d27f99c125389f9ffd2cb85b752445882043824f7d5427edc00838ce00417f9",
    "apply_certified_merge_sidecar_closed_prefixes_with": "8a4969958909c1f7b00e17c71597c4f731dbd3eb0803dfc1a60af0d5250a158b",
    "retry_exact_output_and_apply_sidecar_admissions": "3f05df2b0b705f2adb01ccb3b21de1c2422d947b6f05fb591e316a4a27895422",
    "dispatch_lane_work_effects": "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c",
    "retain_active_owned_reply_routes": "bafe4c316b7d50e5b89bb9468dcf47271985b5f17f8277cb7c70bac5df74be87",
    "retain_active_owned_reply_routes_with_snapshot_hook": "c52a63001d4b73ccd7f06bb0527b7eb4481e29a8ea00be8beed24d841093d212",
    "dispatch_lane_work_effect": "4f26246db63b064c5b6f6389e9960f36968df9861ae9520d911eedbe4c5b317c",
}

# `asyncNodeServiceDeadlines` is a proof-only projection of this one explicit
# trusted runtime contract. These complete-item seals bind the structural
# production half of that boundary: one local serialized height loop, finite
# service batches, and finite idle waits. They intentionally do not claim that
# Rust proves host scheduling or I/O latency; those remain the trusted half.
_RUNTIME_AFTER_GST_REQUIREMENT = (
    "After GST, each non-crashing responsive validator participating in the "
    "active-height or exact historical-recovery corridor has an advancing "
    "local monotonic clock, its serialized height runner is invoked within "
    "the declared service bound after every finite wait, and its admitted "
    "local fsync, signature, reconstruction, validation, and application "
    "work terminates within declared service bounds"
)

_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256 = {
    "V2IoCertifiedServeIngressReservation": (
        "3691c0fd33c09e59c339ee7cb869b2bffd836cd4361bb2d2259c07e6013f18f8"
    ),
    "CertifiedServeProducerEpisode": (
        "e1e0bdfc4854c5553d5fbf70a67a153998be353b7e7016e890af3bbe9a76a67b"
    ),
    "V2IoCommandQueueState": (
        "e64f72be9cd74df96058c1e86ad2b59d94806aabefec83c18c85b49fe119b107"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256 = {
    "barrier": (
        "bce35afb422de5e3b0b7a2667bea940f9170f0ba95dfae03bbea01c36de5a9ad"
    ),
    "matches_barrier": (
        "d4ba831e00f4262e51db685213bd83bbdac6edb12ebc691b9aaa62774880a3b8"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256 = {
    "CertifiedServeProducerEpisode::drop": (
        "926626c686c14d453e18bf20dd7232e33383b26b814178e2d2b7e2c2beb161e3"
    ),
    "V2IoCommandQueue::reserve_serve_ingress": (
        "4b67ec1226b66a4aeb9453e0c956babe5d5a73612a43ff09e63901b47ddef235"
    ),
    "V2IoCommandQueue::try_begin_producer_episode": (
        "da3de6fd057d35073739151cff96230de08a2669e29720423d9b6b832693dbfc"
    ),
    "V2IoCommandQueue::suspend_materialized_serve_barrier_for_runtime_predecessor": (
        "af55e56ebf4bffef6b765e06aec105ede58a1be56f9612c310799fe67aef2ec1"
    ),
    "V2IoCommandQueue::serve_barrier": (
        "1104236dd8cfd0ccec5ea0f3171b9d23c6dc1b0f257f0ec13ff6fe8685ad7d4e"
    ),
    "V2IoCommandQueue::claim_serve_runtime_episode": (
        "776649d85722700536fff55e1af781a66aa8694492c550ba3361255d516d0c2e"
    ),
    "V2IoCommandQueue::serve_runtime_predecessor_capacity_available": (
        "203786f0b005ec7cf105e7189d4f05a23efe71d241c1cb16b3a700c2d4036733"
    ),
    "V2IoCommandQueue::finish_serve_runtime_episode_turn": (
        "02d2bb266994d99ca4fdef5ec8d90fef193a969fe2136eb6e0f0a7f0ae47f0df"
    ),
    "V2IoCommandQueue::try_send_as": (
        "6c7a0afdfa074b745803704e6910671ab28099c9bc294c21c5fe67a55fd3905d"
    ),
    "ProductionV2Services::certified_serve_barrier": (
        "2daa1fafb0049a95cb78d01630aa55932539b3de9a0db99eb169c0e5e031f56d"
    ),
    "ProductionV2Services::claim_certified_serve_runtime_episode": (
        "3b2fce586ae8d9b59bba1bd4f196ae8e9f5b6fcf3d65ae21d03c541480aef5db"
    ),
    "ProductionV2Services::certified_serve_runtime_predecessor_capacity_available": (
        "b995b96761597dfa1f580742c4898a1ee4a365a627cbaf98203435c33f252ed2"
    ),
    "ProductionV2Services::finish_certified_serve_runtime_episode_turn": (
        "2467e4a47772c21034f08851f53fcf55cd34bc642bfb8f4f92b519cafe2524bd"
    ),
    "ProductionV2Services::try_begin_certified_serve_producer_episode": (
        "b86f953644efbd6a73d81bba2a3cbc23195cc9dcb68e44bc90e6b9a4a8cfd15f"
    ),
    "ProductionV2Services::take_exact_serve_predecessor_completion": (
        "08a9eddfb80c1b792988ce4433b0f200e2dccd9a8498b9c8869feb3758647307"
    ),
    "ProductionV2Services::drain_exact_serve_runtime_predecessor": (
        "aa2d628cbb2b5f54d0901f9949d3581d6188efe72a3656958e95e9e9dacbae92"
    ),
    "ProductionV2Services::drain_completions_inner": (
        "d76669b25f89b35aa41b771a85f51e398558709d4b685de5e09339d530478924"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256 = {
    "advance_executor_once_before_exact_serve": (
        "0d3580e1ced5446597465ca2dd3a177a16dc168b66b8b8c9e2350b68fb03b86b"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256 = {
    "publish_external_lifecycle_owners": (
        "0b872fdf4ad76a4b092cadc6cd33d041463153adbe0d67724b7919767e7f46ab"
    ),
    "older_runtime_lifecycle_predates_exact_serve": (
        "55cb9ccb720be2208ecd9c3e19b4e514dc8072022f2567b663e670e5d153e53f"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256 = {
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "204a88b77ef7853327a2313583c25ef32f9bdea705dd57e0fc7bbd17b30afa1a"
    ),
    "active_lifecycle_uses_ordinal": (
        "3383cbf7969a7f8d792f56ed9c148c4ca523f6366bfac8c60b53a68af427a4fc"
    ),
    "older_lifecycle_predates_exact_serve": (
        "2b832129d3f5e91ea0e333400277c9715785f896b460b0de55e398d2cdc69b30"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_INGRESS_ITEM_SHA256 = {
    "oldest_active_lifecycle_ordinal": (
        "fa6fbffb7201f69529caec900bafba3eb4f9433f00ec31289ca3e23e2363c7d4"
    ),
    "uses_lifecycle_ordinal": (
        "248f70bfe769734211acc566c8b91b7e0faf037990be47f95b231d5f102a557a"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256 = {
    "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io": (
        "51623ab6cf102b45554a8135cb4fbca35062b0d5db6bea99cbabba0eb2305d9f"
    ),
    "repeated_exact_serve_claims_close_all_older_sources_before_later_io": (
        "e97bed021406e0a72e692d2ac6ed58e491a4826b5a9054f17ca8a64d95ef0968"
    ),
    "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission": (
        "ebeeb9606074ce7e7d51aea4d55d54dabacd23435d639d81ac6f0a9849616e21"
    ),
    "worker_completion_is_retained_behind_a_full_runtime_fifo": (
        "4e807f3dd45e855a96923c803bd970dbc2a6684583b8dfa84aa6db788c5206c1"
    ),
    "production_drain_publishes_worker_completion_behind_full_runtime_fifo": (
        "913796745fa6c589f20dd309da632c1dfb05f84aa963377b04ffa9a0556e21eb"
    ),
    "drained_exact_retransmission_gets_fresh_scheduler_ordinal": (
        "6604f22a559d4217374ee443dc7d30641756b0025cdace2aa0fc65e7112f5cd4"
    ),
    "certified_serve_future_slot_blocks_control_and_consensus_replenishment": (
        "4eb2da42d968642c6eaf184f0ddda6f726e4a7fe49d6b6c2499092ee3c97d075"
    ),
}

_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256 = {
    "restart_dormant_local_fifo_reservation_survives_full_class_churn": (
        "653fac9ef34bfb376711f643fe231ee9860cb7c1ef29570d514a3f870a531c1c"
    ),
}

_LEADER_WIRE_PHYSICAL_INGRESS_REGRESSION_TEST_SHA256 = {
    "restored_productive_retry_freezes_the_current_physical_source_prefix": (
        "0717cee735d0b0a435bdcd502ca6e29c87a554a9f017cb7edbf1efb46ef33d20"
    ),
    "restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire": (
        "e994c3391ce2626972c26c57a37039f71573adb9d82bbcfa64619edb70640255"
    ),
}

_LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256 = {
    "fair_v2_ingress_admit_leader_wire": (
        "140325f2b05a0c93744b2fbd483e485d95f17a0c8ce08ac1bf41808047e306f4"
    ),
    "try_recv_if_at_checked": (
        "24f0b8b8678c7ca64f3f46b2d214553be166e6a2bb2d8ec21f3de8968a35e7ef"
    ),
    "ingress_scheduler_ordinals": (
        "994beede48b0f3f8b0418f2eac37029ca5f65fc934aa4206e9dfc69d1a2acefe"
    ),
}

_PRODUCTION_LOCAL_RUNNER_SERVICE_ITEM_SHA256 = {
    "run_inner": "3d8911e69d491268dabae08f5c6b996ede417dd8afbda670e80dfc1070ed2271",
    "advance_executor": "321df6c9713c5fb64fa6a0948dff6464afa3d08040e752ae0e19196b9badfb31",
    "advance_pending_tip_recovery_executor": (
        "a85c018053d4b47dd1c36194a66318422f72eb80e3cca3ac2ba9db5f44eeb9dd"
    ),
    "outer_ingress_turns": (
        "a5408a498d6ee0837e31c5fc1b152c043b3d2930eda580d750eaec4f2704ef18"
    ),
    "apply_bounded_sidecar_admissions": (
        "27eb4ede4dd038babb38255b89f6a25259b79f55c6dcee33779efbc5d91e04ad"
    ),
    "dispatch_lane_work_effects": (
        "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c"
    ),
    "drain_lane_relay_ingress": (
        "665e0ea1c01501d80a547ec3d4ddd72117d32f7ea748de4ee2d0803519afbfb6"
    ),
    "ProductionV2Services::drain_completions": (
        "dbdb63d50e19b3dfe3617aaedf53e7d7f13c105c4b844c5367d31487fad10ea3"
    ),
}

_PRODUCTION_WORKER_ACK_SEAM_ITEM_SHA256 = {
    "PendingExactFanout::classified_with_route_history": "a1f9ad9102ffddade0c83bac4895a781a72ba7bec980d99e914eb0641dafc9e7",
    "PendingExactFanout::classified_with_reply_routes": "84c22ccbeecfb531f69c7a03b6f659281fb69fde25f013e0aa394afba347914a",
    "PendingExactFanout::retain_active_unowned_reply_targets": "d34c8c2d4f0f4e402bae740f6463445a30453b88761fcb125cc5ac506479819e",
    "PendingExactFanout::reply_target_merge_plan": "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653",
    "PendingExactFanout::reply_target_merge_plan_with_hooks": "570dd53c526360494d3ac589a2b6f4dbb7c4da81610ab7e57cac4dadb4d20766",
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::retryable_certified_sidecar_responder_control_target": "6ec5aac36548c23c407f8212aa93e9794a1383f4ce20c517ac6c158f24193cab",
    "DurableExactOutputServiceOwner::is_sealed": "e739a883ce93dc761dcb9ce0673f69e2e280562ab67f3ca94d1a454efa0860b6",
    "DurableExactOutputServiceOwner::seal": "abe0ec8f7915faab4687fa7be6606ab69e8b58f650cf4ba62bf8610eb586e652",
    "DurableExactOutputHandoffReceipt::is_bound_to_transport_owner": "3a991fb6c233282aab23f9e1cf30857457983736bbd15c7da0f25423eb76a2b6",
    "DurableExactOutputHandoffReceipt::matches_predecessor_context": "01cf51ba83773bb7a1b7e20bb17581df680a53f8280dcc5a7afd35663be1f502",
    "DurableExactOutputHandoffReceipt::matches_finality_artifact": "2200062928c7e969f4793c6051724bc806ce4ba8199dc439a0582a6e24b2da90",
    "DurableExactOutputHandoffReceipt::authorizes_immediate_successor": "e99e80f5fd6b36abbb95041e2b0c5ba71e435e11fed7ce456418727f6870ab2f",
    "PendingExactOutput::new": "4d6f8872fd25a2d6041dfaab7d2182d3b43d13fcae408b478fab17f34afc738b",
    "PendingExactOutput::is_pending": "6cd7ecb71f163b7b59e59abcce7f413a4eef60bfbd160950c4afd03a0ff73588",
    "PendingExactOutput::close_certified_sidecar_prefix": "d7e50f6894a043a303ce239e0f37dfbd7a12a7c9a621f0e48f6754a72739c3af",
    "PendingExactOutput::pending_sidecar_flushes": "14c74fcdfe37c137fe20897ab19acbac746b9d3a4f764a939abbbe1bc43b2048",
    "PendingExactOutput::sidecar_control_units": "e5eeb08ba9065c86d9b13abbf45b991864d538339f09b2dfe42e5afa0b84ef68",
    "PendingExactOutput::restore_pending_flush": "ff20f9c93c8cbab55fdeef391e73223cba5bfbc0bf4fac2b9c8130b984ac0d7e",
    "PendingExactOutput::poll_reply_flushes": "eae8ee4dc4996b077b9d0e3315e96e8c35a18b0189f2add40e898e60a4167749",
    "PendingExactOutput::validate_owned_reply_transfer": "c39a07ac424ad25dc1d2d1d5cffec3daacbbd7a83c029ff289739745acb6f591",
    "PendingExactOutput::can_enqueue_owned_reply_transfer": "8fcf9c1dc5edb24b0001104718bf80ff142716e45e36b179f74a430e2b214c38",
    "PendingExactOutput::enqueue": "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6",
    "PendingExactOutput::enqueue_owned_reply_transfer": "285ad0dcddadc2a95c155a520f636d14932f6c9c2913e7eea7e43cf443e6642b",
    "PendingExactOutput::project_sidecar_receipt_completions": "e506155b90096fb5980cb878611a5c42190591efebcd7d10d3961061cf925158",
    "PendingExactOutput::retains_retryable_sidecar_responder_control_for": "8f2960dccb011be19e3cb0135ba26138f4020dbc91333bbcd1a73cdcae8d17c1",
    "PendingExactOutput::enqueue_validated": "035f6caf6f60eb2fa8c6a2da471fd60d6f4692061815906378aa115ad29cb92b",
    "PendingExactOutput::handoff_applied_height_to_durable_reconstruction": "e78d702c927524d363d59d1a098bfd6649d6d399e3f0252faac9d500c77b5a80",
    "PendingExactOutput::drive_with_budget_ack": "a4453951bf9775ad83b603cfe9b5f5849cc6251904602cbf76438c6140703bfa",
    "PendingExactOutput::drive_bounded_with_ack": "7e630c6be2466fe0980df81ea194e101b70e4abac9546a379ab9f24c8eb23861",
    "PendingExactOutput::park_unwritable_reply_target": "bae33ea7bcf13a905da400e913bed5bf2347f103d52be229e0d57fc6f24376d2",
    "ProductionV2Services::admit_network_exact_output": "8652c4a435eeba2522055d198d1bc997befa05615097688986be0d4cb0d1f460",
    "ProductionV2Services::drive_pending_exact_output": "ff02d49d849445526f4f4571a0544f96ad113b257a49ba682a39c6f03e88b950",
    "ProductionV2Services::enqueue_owned_exact_reply_routes_while_guarded": "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff",
    "ProductionV2Services::retry_pending_exact_output": "a826dc12f81db5e245b5a3eb1d94d89f43ecdf61b2e49f594d4f0c696594b31a",
    "ProductionV2Services::has_pending_exact_output": "107becdd3b504739250f12867eecdb459362f907957797ea3a0e31a71e360768",
    "ProductionV2Services::drain_certified_merge_sidecar_chunk_admissions": "d065d7b2b625852ed7ecd8458997c763b8c6d4d1ef445275150c16a62f1badf6",
    "ProductionV2Services::close_certified_merge_sidecar_prefix": "fb6879c8c325aedc7c19f093a9e5b2cb1b8c25c7bd89c7da1f2694e923cde3a2",
    "ProductionV2Services::can_retain_lane_work_effect": "e0bc2cb14070a0443f7fa5d44ca11d7e4d78106af0754bd181e86a8ffb0eeb3f",
    "ProductionV2Services::handoff_applied_height_output_to_durable_reconstruction": "2064071eca55b70e01a51bda015e5cda8561a35d2f0d9e09559ac23c01ae6f8d",
    "ProductionV2Services::seal_applied_height_output_handoff": "95138e27f7dbcfe7e84246edd8d14acb3677adec2e0b6ad0c80e2a65688a4a62",
    "ProductionV2Services::validate_applied_height_output_handoff_authority": "7e122f2997f6fa75a67033addc960c72e2dad0dd766588c2d667e056b9363bc3",
    "ProductionV2Services::finish_height": "2700da04492e8587a8934f3d037d05e8176746a4a52790a29b185560c20af360",
}

# Exact token seals for the frozen validator ownership-unit geometry and
# independent authenticated-source attempts. Qualified keys are used where
# common method names occur in several worker impls. The semantic checks below
# additionally bind atomic per-source route merging, non-regressing cursors,
# roster x class reserves, and shared-unit accounting.
_PRODUCTION_EXACT_OUTPUT_RESERVATION_ITEM_SHA256 = {
    "PendingExactTarget::apply_reply_route_update": (
        "e75cca425a7afcbd98a0eedf225f6586dd333a06afbb38366e0660b591a84fee"
    ),
    "PendingExactFanout::classified_with_routes": (
        "c848ccf3f2e54014325dc8b27794a5f9771a7df45f6ed9aa16ccc03451145d18"
    ),
    "PendingExactFanout::target_source_at": (
        "1214b0b8088d93df32646db104442dd46d56dafd53b4cceb14afedff54629cd9"
    ),
    "PendingExactFanout::outstanding_sources": (
        "0acccc51a9bedd5fec75171eada06cfa27036daa8b29d74dfebe9ee8a65f0078"
    ),
    "PendingExactFanout::outstanding_sources_excluding": (
        "5ed556d54c561a57c50d7636f84fdefaf29e14d1179de7edc245447ca861f1be"
    ),
    "PendingExactFanout::outstanding_reservation_counts": (
        "02972f0bfa1e31b0dca617382be3254a8da07177396c69101e068cb02075b3fc"
    ),
    "PendingExactFanout::target_reservation": (
        "94273ddf470c326dea9ab618150c7611ebf353a5f2358339bd52b3e382335bcb"
    ),
    "PendingExactFanout::certified_sidecar_topology_progress_target": (
        "fcb909658b3a0e546a8e6e5379ca4437ce4e55c2262f88e7ebb59ab7d8ff428b"
    ),
    "PendingExactFanout::admission_reservation_counts": (
        "3b5e7800f133b7673650fee75acefd31e56316c2d33ea5405c2b898fd11a8659"
    ),
    "PendingExactFanout::reply_target_merge_plan": (
        "d4470015919d5a1d7838b0c33a8a8c2c545808eac2e73e7ed8e5d37db2be0653"
    ),
    "PendingExactFanout::reply_target_merge_plan_with_hooks": (
        "570dd53c526360494d3ac589a2b6f4dbb7c4da81610ab7e57cac4dadb4d20766"
    ),
    "PendingExactFanout::coalesce_reservation_additions_for_plan": (
        "081e95192953390b7b1b17794f956c9e089d7986bda1c55ce49ddd83ad4f0979"
    ),
    "PendingExactFanout::preview_coalesce_plan": "f911b55bc95a4ea4ad07902c651cc090ebe5fb2d488a4adabbd86e22e7b720b2",
    "PendingExactFanout::commit_coalesce_plan": "a311d1d98cea528b88438f90a8f1d6e87b5adbd1d1b158730765b4e66239cade",
    "PendingExactFanout::can_coalesce_retry": (
        "b5f48359f0342b142f04b6fee8c6e74e7cbeaf5068f1dc628fffe7ad5a971de5"
    ),
    "PendingExactFanout::has_writable_reply_target": (
        "a717b59e74c10dda4248c99527c781463eabb0c8ac355f2e718f5b3f3e0f0b9a"
    ),
    "PendingExactFanout::is_stranded_retryable_certified_sidecar_responder_control": (
        "ce27592a9a7e240a207c1b0ea26340edb7e7f25d11f15bdb7ef3d0698446ef49"
    ),
    "PendingExactOutput::new": (
        "4d6f8872fd25a2d6041dfaab7d2182d3b43d13fcae408b478fab17f34afc738b"
    ),
    "PendingExactOutput::ownership_addition_load": (
        "c566f3dc97560d01457335a290f876f96f5128236bf8cdbbeda6c0c6d14e50ef"
    ),
    "PendingExactOutput::ownership_capacity_available": (
        "078ce17f80c831c8018269a75c2955852a9088e4a150eac969bbab05fc0e2119"
    ),
    "PendingExactOutput::source_fifo_owners_after_fanout_replacement": (
        "2826119bad851abb20d3e7f838ba7df428b9015ec26a448ab0c5536fd3c0fc3a"
    ),
    "PendingExactOutput::ownership_state_after_additions": (
        "a22e2ff976a86ec6218c8a417b2f9b3fd066d16bb9b8edd561fcc00512008ad2"
    ),
    "PendingExactOutput::ownership_state_after_replacement": (
        "80d59ecc4e9300743cf22d82a07712f9c585cbda0cbaf49197ff89bfefc66932"
    ),
    "PendingExactOutput::coalesced_target_geometry_available": (
        "16637dff951694ae9999357a0121a18e520dda07935fb88839625d080e47f403"
    ),
    "PendingExactOutput::remove_ownership_units": (
        "028bce9d7aa7c84ad347212ecfdb1b76066ac80fa9e713219a89e590ef0715bc"
    ),
    "PendingExactOutput::validate_fanout_bounds": (
        "1b40b3085feb03a957e28a2bb7776ce874cd571f0206148aa00126a7dd57bea6"
    ),
    "PendingExactOutput::capacity_available_for": (
        "de94c2648c59bc1e9a4b0ec3c5f4824ad7bdedf153012603c6ddf407c42326e0"
    ),
    "PendingExactOutput::can_enqueue": (
        "21e501bc34a7c1b41ca787e5a8c4a48d9e853a8cf7b3960c685ef58e7d33cc0a"
    ),
    "PendingExactOutput::stranded_responder_control_replacement_index": (
        "5a93e8d4bcf188928987ad45e193a7569f25c68f05ca9d7f25a93ba3e96974df"
    ),
    "PendingExactOutput::responder_control_replacement_ownership": (
        "3b28dfe491d895c31600f4c6f871fb04015f7a9214dbb6768431c8484bf6dac6"
    ),
    "PendingExactOutput::responder_control_replacement_available": (
        "1680843d65a96ef27f9c5d68c968dbbea10998b7641194647adede693c3fb4c7"
    ),
    "PendingExactOutput::responder_control_replacement_plan": (
        "1a914ee0f02c0e45e7297282a6a9d415761d610ef303665bd98951ef6136fb79"
    ),
    "PendingExactOutput::replace_stranded_responder_control": (
        "5b87149fac9b75e1690787b5167ed04681676a186f0bd2bfb834ecc5a3c748ff"
    ),
    "PendingExactOutput::enqueue": (
        "b6379336a656f578037f65bb7b297529092f3f64c06bf38ef5f590a4a3aa81c6"
    ),
    "ProductionV2Services::start": (
        "2d10b4ff066780c14b2b94a0313850563a2b1eb4829c9f998936d570efc8de40"
    ),
    "ProductionV2Services::exact_target_geometry": (
        "978520459f9dd3c5459478e222418ffed2924445c40a79722c307f97e6d28871"
    ),
    "ProductionV2Services::can_retain_lane_work_effect": (
        "e0bc2cb14070a0443f7fa5d44ca11d7e4d78106af0754bd181e86a8ffb0eeb3f"
    ),
}

_PRODUCTION_DURABLE_HISTORY_WORKER_ITEM_SHA256 = {
    "durable_history_source_covers": (
        "c33f30a9d2e34b9860f96ba156788f3a6289776da59a45eb7c2fa717b5b90299"
    ),
}

# Typed semantic claims are the production authority which lets exact output
# cross the applied-height boundary.  Seal both the claim validators and every
# production constructor so an untyped producer cannot hide behind the
# scheduler/retirement digests above.
_PRODUCTION_EXACT_OUTPUT_CLAIM_ITEM_SHA256 = {
    "covers": "48529c793eedab83283aa2d471486e8bbd77ce399c0be52576bc23ee3b8b540c",
    "from_request": (
        "790314790a852bac49e79b6c71bbe07931ade269d273c536978b45f4822a4b68"
    ),
    "from_chunk": (
        "73871d46bfc5ff28b4db03bef64811ee26e995795dca98c552f5dc8b1e4067f9"
    ),
    "native_amx_message_body": (
        "90737e4116833fb086b7c1f3a7a04dbb34fc9157385103f3dc1be6e74c127eae"
    ),
    "scope": "f86a8297b6cbfb9042d6fda08cab69dcc2ff2e8c64455982df70b249afbfcf64",
    "validate_fanout": (
        "26e909f5558df72e2f13643d356ae8788175375cb37dd4895cfa0b3f8d60a8d2"
    ),
    "claimed": "75dcecc8adae80ad5980fb812e4d13af3e7f432605c685985a6c2d0e27a67a10",
    "claimed_with_routes": (
        "b4e6d00e981ae28935489541d49c181941a4a38c1384a823ffc7d8ace27e9418"
    ),
    "enqueue_exact_fanout_while_guarded": (
        "8f222f8dc1b6421990600f5a59d489fef0452793c2c9e4468296c092629d1d2d"
    ),
    "enqueue_owned_exact_reply_routes_while_guarded": (
        "e26271d8dee4d4a3edc25b6619ca182965b3f90b9b38f945aed1c4b0c632a3ff"
    ),
    "drive_pending_exact_output": (
        "ff02d49d849445526f4f4571a0544f96ad113b257a49ba682a39c6f03e88b950"
    ),
    "exact_output_scope": (
        "2c322931cf99b7f7e6484c11c48b4bb570b48bc6a95f240e09fab88eea599be0"
    ),
    "post_to_peer_on_reply_routes": (
        "326e01fb46a4f99e7bd8c3b09f3216252b74ee7448003a2640dec578e4a8c08f"
    ),
    "post_durable_history_response_on_reply_routes_with_permit": (
        "c2f9ac9b07a52814d6ba43a0701ecd6a04cb6ca7cd890efb100330b958c0d651"
    ),
    "post_durable_history_response_with_routes": (
        "a318b9a3e2823381613bf87f911d1efd3721a62d9a6a35e4b2151396672363a5"
    ),
    "post_lane_block": (
        "c71bddecf7e1a3891256eaf7ced7c37adb7fc196617b4f79a7f17c6fcd6ee902"
    ),
    "post_durable_lane_certificate_on_reply_routes": (
        "84b9fa1c26b45f636ad7fd2b605b3cb4398a74abb3f515ea9e05034ca7a9e429"
    ),
    "post_durable_lane_certificate_with_routes": (
        "27bd2d06ec38071aaee1e45c9fa21ee51205e34dcfbb76f59d4b2cbf906c3fb8"
    ),
    "post_certified_merge_sidecar_with_reply_routes": (
        "333334838aaf7761315f2eabb1f34e6cead127ddfd4f1e6282ae116e5c9846bc"
    ),
    "post_native_amx_with_reply_routes": (
        "17d66596b19902ce6ac3da41c7c05a000b42a54a87320f92cb588500f151c396"
    ),
    "broadcast_merge_to_voters": (
        "99b0c80af2876f9b92cd9789605a7040d3e6dec0b4ab11f47edf292aeadf5f59"
    ),
    "post_block_message_while_guarded": (
        "ff12cb7fcc62b94353055da1bab287edca685fdfb4558f8c91872c238ec75b23"
    ),
    "post_block_message_on_reply_routes_while_guarded": (
        "f3386e036a83ba81782e46c9eeb2fe948366907c9bbd510f42b646bf4f1ffaae"
    ),
    "broadcast_preencoded_to_voters_while_guarded": (
        "1c22c254ba300a86887affe725dd01e126563ac584f791e9da329fe67296bf4f"
    ),
    "broadcast_to_voters_while_guarded": (
        "bbc43f18d7ee7cc95ecfabc70894ae1b82780edab21ebecb8a60445e68113a4d"
    ),
}

# The lane authority is the only production witness allowed to supersede or
# reconstruct lane-local output at global-height retirement.  Bind its exact
# Kura/application source commitment and its winning/non-winning validators.
_PRODUCTION_LANE_ROLLOVER_AUTHORITY_ITEM_SHA256 = {
    "covered_source_hash": (
        "a72992ca12a20a2f7ed989229536bc83895cf7deed919643b3479758ca9b34d9"
    ),
    "persistent": (
        "3e865b8a1b9b7136ab2cf81dd6d33fe84ae9e9b14eab2006156ae9cbc29520ff"
    ),
    "lane_output_identity": (
        "bf17d20ee94a5023ce4623b31eb333e8d7cb12c1a155d058a2a9f22840430b58"
    ),
    "validate_winning_lane_output": (
        "5944d18c4054801e9d25faed1f67317cfcd9d0fe3e8dea1cb17ba35de5566371"
    ),
    "validate_winning_lane_qc": (
        "345e7f2f2faadd6dc57195390ce8e711549608ba56f1cb01f962250414f4c011"
    ),
    "validate_superseded_lane_output": (
        "ec480ca71d859f5b27fcc31731a1bc2ebb02ee8d5002475d53d3ed14109c94c2"
    ),
    "durable_lane_rollover_authority": (
        "3a011b86998b68c9ae94f028bd373a75965ac050352f45c510ebfae52c8bc3c8"
    ),
    "serve_durable_lane_certificate": (
        "bcab2428b4a2d43dd23989bebe917077e84b069c4e120808f6b25ce4503ce52f"
    ),
    "reconstruct_durable_lane_certificate": (
        "adf988938c94e9869ec4f26c9942cea89fb0cc0669677688640f9c7567dc2a59"
    ),
    "reply_routes_are_live_for_peer": (
        "cdca18bef9df99c77e3698622c9cf6941dd249967bb587f60e9fd381a4f8b235"
    ),
    "lane_work_effect_reply_routes_have_valid_shape": (
        "5140d1973a6992f34c59d93739d2fe62cd3e6115998963a6fdd3a46d142ee83f"
    ),
    "lane_work_effect_reply_routes_are_valid": (
        "893db592b49209bff9a7ea5ee430acc76390ade30a1a765ca7ae19b1b5806a31"
    ),
    "merge_optional_reply_routes": (
        "39e76e7cfe0d234d3508852537e442bb6511a51f372cbfded1bf6bd0ff853f2d"
    ),
    "optional_reply_routes_retain_candidate": (
        "5c43c5b723d77fdd98b194a0e86ab37c25c3a165c4aa96c032654238863bca17"
    ),
    "merge_lane_work_effect_reply_routes": (
        "0d32e263ed0d2659f4274b03a1f8bb87f8de386beb9afb8103eaf3342c6b4459"
    ),
    "merge_lane_work_effect_reply_routes_after_route_merge": (
        "0d0e5c73d9a47d0141fd1038cd81805a4d6e0aba1655e0caf620afc0db2fa616"
    ),
    "lane_work_effect_key": (
        "953b1b7b5464d9a574c4ea9d3b15cde884320bde2f49efe57f8fc4fba63438de"
    ),
}

_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256 = {
    "run_inner": (
        "3d8911e69d491268dabae08f5c6b996ede417dd8afbda670e80dfc1070ed2271"
    ),
    "drain_v2_ingress": (
        "ec5d2c9208f78350ab96b0fea82460a6d8225cd220d1d99901bbf866dfb7ad2b"
    ),
    "dispatch_lane_work_effects": (
        "7b7c0358e9fa35a05df7acd0c641b693b01b51926be2180ba02efde110ef774c"
    ),
    "dispatch_lane_work_effect": (
        "4f26246db63b064c5b6f6389e9960f36968df9861ae9520d911eedbe4c5b317c"
    ),
}

_PRODUCTION_CERTIFIED_SERVE_INGRESS_BINDING_ITEM_SHA256 = {
    "CertifiedServeIngressBinding::bind": (
        "4033d2192ddb54c72c444ba5a53f1d0bfd04de32dbbe78582f57a6abe4b8b013"
    ),
    "CertifiedServeIngressBinding::retire": (
        "eb6d1d6a225610f182464077a030316af3cb66a95322f504b76c754134fa6bb0"
    ),
    "CertifiedServeIngressBinding::drop": (
        "88ddf5ff64ea693fbb027a716505a017236eb44e435eced813a6e4f6fc9bba61"
    ),
}

# Exact source seals for the configuration-to-worker capacity corridor.  The
# constants are checked as exact source tokens below; these item digests bind
# the checked arithmetic kernels, the user-root call site, and production's
# narrow refinement wrapper to the same geometry.
_PRODUCTION_EXACT_OUTPUT_GEOMETRY_ITEM_SHA256 = {
    "actual::sumeragi_v2_exact_output_shared_ownership_capacity": (
        "e71d1a2376fc34aac057abebde05a41c5b32bbcbad17a409c954845bc4aa64ef"
    ),
    "actual::validate_sumeragi_v2_exact_output_geometry": (
        "b9ad00e3d2ee76b202fa98f53cab9f7264c63a7d9a3050b0c3d57c0449cfb8f5"
    ),
    "user::Root::parse": (
        "61526de01c92467504c7b92924b7fea8e13db44f81d385cd226ae644bfe11c7c"
    ),
    "worker::validate_shared_ownership_geometry": (
        "67026793b1424da887ccec0301157480e43b3585d298ab1545fe98e8cb577411"
    ),
}

# Complete token seals for the process-local fair-ingress carrier and every
# exact-output consumer which must retain it.  These close the route-only gap:
# equal payload bytes cannot substitute a different semantic origin, and
# alternate authenticated sources keep independent non-regressing cursors all
# the way through runner, effect, worker, lane, and sidecar service.
_PRODUCTION_EXACT_OUTPUT_INGRESS_SEAM_ITEM_SHA256 = {
    "ingress::merge_downstream": (
        "9542ecd100449b693ebae4c1dbea39f43d651c57533ece26dccb193eab6f77bf"
    ),
    "ingress::merge_downstream_with_observed_receipt": (
        "c097d227ef0a670c2144dffcc30a6a0b18016b781ba6a51728c21b2ee8bf32e2"
    ),
    "ingress::merge_downstream_with_strict_receipt": (
        "e65283e5924e1dfe938cb69289a35067a01d3835027ba65587a32f515ed90032"
    ),
    "ingress::merge_downstream_with_exact_routes": (
        "9937dd446dc210e96e72e558afad2736ce42036951fc761c432c3c53d510cea2"
    ),
    "ingress::same_semantic_request": (
        "b30874c2662dc448ae573d60fb9892310817975b35dde2bfe5d389d08ada7730"
    ),
    "ingress::matches_message": (
        "6dab8ef36de2046cada66b6aef1a4db08e458e0d43fc3862f794002192c58398"
    ),
    "ingress::matches_semantic_origin": (
        "222fb8e00e39a211f73c88d8c7a41b83a82897c7428f70339aa38e40f78da719"
    ),
    "ingress::process_local_projection_hash": (
        "695a14164f614347818728fd5b67e421e881a5e2da02f513448ad7a54ab2a816"
    ),
    "ingress::matches_reply_routes": (
        "98580b7c9e69cc4c69cc32ec6212459e93fba573a335151fec476659d2772c2f"
    ),
    "ingress::project_retained_reply_routes": (
        "6a95a61e3dd7ce8da838fd99693205618d3ec3979ea88b46599ecd68ebfd2fdb"
    ),
    "ingress::advance_reply_cursors": (
        "2be088efc3ff0460b06c0316f7b64062d864d22d8fce286a1474fd1e31af3c82"
    ),
    "ingress::validate_exact": (
        "526d02c98338c0304ea246b5366e29951ed312a57e795c4a2db4433f06d1d66b"
    ),
    "effects::accept_payload_chunk_with_ingress_ownership": (
        "04c48786b1db18841d32be400432570fcd268452506af9735c2dc5354a72e5db"
    ),
    "effects::accept_certified_body_response_with_ingress_ownership": (
        "c251ceacf29130bde403f38b7002321317c70dde04764f499bae7cfb7b15b722"
    ),
    "worker::claimed_with_reply_routes_and_ingress_ownership": (
        "8c95a2604897a3dbf327d36721388bf466784f2203869d1fc6aead594ecd7e44"
    ),
    "worker::serve_certified_request_on_routes": (
        "6cfa46d3647ba72ae53fa12585926eb6803ed38dff9d656894ba7e7693dc06ac"
    ),
    "worker::queue_commit_serve": (
        "73b2b40e5c2255d557c2e67ccdd58c3593381c5d713b4dd6315e48c350ff7dfb"
    ),
    "worker::io_handle_certified_serve_ingress_gate": (
        "87acd6865d179d3911a66692e063ece997fbf7669a81493b19d9f443e1291735"
    ),
    "worker::services_certified_serve_ingress_gate": (
        "95906877d32686cc23249747ff5e31f8552d5abb237277f96fd4091da1a92d77"
    ),
    "worker::route_payload_chunk": (
        "cabbad53c21b8b42e361f14c99a045a327c20e473ebb06e189ba82925871eef4"
    ),
    "worker::buffer_orphan_payload_chunk_inner": (
        "03c11a6a6438f3a0f7cf47cac8f5310b262fd684a9df7891ff906b7f2e0d7de8"
    ),
    "worker::replay_buffered_chunks": (
        "55be3d8d05cc8924f303cac80b89e5d56b8b6496694a9e94f742d17fbcb7a80f"
    ),
    "worker::deliver_payload_chunk": (
        "f4341f1f806d10adab9e14b0c4b13c463fc7c6dae22bd7056863548d0912fb5b"
    ),
    "lane::accept_lane_message_owned": (
        "4c3b13ab1d0821d97604c8f9119ecc27d1c5be5c2a635b228027d35ed3adf96c"
    ),
    "runner::v2_ingress_head_can_drain": (
        "d4e61362952b96d782ee41e1c6081a76132086b8e82cf430f8c0003deb8dbe70"
    ),
}

# Exact comment/literal-free token digests for recovery-scoped eager CommitQC
# discovery. These are production source-fidelity seals, not machine proofs:
# the corresponding starvation-freedom obligation remains specified_unproved.
_PRODUCTION_RECOVERY_EAGER_BLOCK_SYNC_ITEM_SHA256 = {
    "initial_block_sync_deadline": (
        "b3455d656ecac4787561951ec55cc8cde44e2a2c90a3330ad3fe0c65e96c2185"
    ),
    "retain_eager_block_sync": (
        "b5ed7336da6088907bcd26f1b21d2e0bd91a972e8bce68a6e190f238d4c2b56d"
    ),
}

# Exact comment/literal-free token digests for the allocation-free P2P frame
# geometry used by Sumeragi height activation.  These helpers are part of the
# production refinement boundary: replacing checked arithmetic with a shorter
# estimate can make a locally valid progress envelope impossible to transmit.
_PRODUCTION_P2P_FRAME_GEOMETRY_ITEM_SHA256 = {
    "checked_len_prefixed": (
        "b6411bf29b1e2517fb2c4151c52634334f712f05127c4efd6440166ee6b65207"
    ),
    "peer_id_wire_len_from_raw_key_bytes": (
        "b13f1926dab04641ff700941aaf854a29a0205e4828831da3f207a0930829844"
    ),
    "peer_id_raw_key_bytes": (
        "8f3464a658389a9eed4a02b26e3117aebe9c11283d825736ea6ef5fe797ec09e"
    ),
    "relay_target_wire_len": (
        "84837f33c9793445071c17cdc11de01ddc1b7b57e061896afd381252743d3c05"
    ),
    "relay_message_wire_payload_len": (
        "76c27a4cf75123379bdd30fd759589189574587442a29333618dd6fa78e6a145"
    ),
    "data_frame_wire_len_from_payload_len_with_peer_key_bytes": (
        "6a7f400c67afbc25c834fdb17f782baf374017cca082295f88bbfda501199fb1"
    ),
    "data_frame_wire_len_from_payload_len": (
        "33df4c34f2034f666dfe403b97869e498094f42a6fe35c5ce0ab75164fe0bcbe"
    ),
    "validate_transport_queue_geometry": (
        "2b43cba3a15fb667169280663e960cb6fabf5ccde7272d4276b6480041c66632"
    ),
}
_PRODUCTION_P2P_PEER_FRAME_ITEM_SHA256 = {
    "data_message_wire_len_from_payload_len": (
        "d157ece83c8e700725549e91fcb572b61ebe3d8c3e267e9d3bfe31765ff3310c"
    ),
}
_PRODUCTION_P2P_RUN_FRAME_ITEM_SHA256 = {
    "frame_plaintext_cap_for": (
        "4d66d6b2dc3c139c4df54c7c9d7b7640691ce3aa0765a44298b689e63d360e21"
    ),
    "checked_encoded_frame_len": (
        "e7d2ca074d7c7eb85d16608018e4e8d30950bc2278edbe52e3a63e31c2b15935"
    ),
}
_PRODUCTION_P2P_QUIC_FRAME_ITEM_SHA256 = {
    "try_send": (
        "bd32ba60bc0de89dc1ac9a7062a69c41df4ff6c72dd5c053550385780510891e"
    ),
}
_PRODUCTION_P2P_RECEIVER_FRAME_ITEM_SHA256 = {
    "reserve_for_frame": (
        "cb0e506080c0985c5f81c5c567250e143d072f2f23e06b4072cb3d2b739a8212"
    ),
    "parse_next_encrypted_frame": (
        "95da490e1c30cfe119fe51a31fffb2b08287efab0e0e3134d7d5face610f3212"
    ),
}
_PRODUCTION_P2P_SOURCE_OWNERSHIP_ITEM_SHA256 = {
    "same_owner": (
        "e6bba8d24c683f3b322752c93d977415e41e8dffcd9221f410d462a4006835db"
    ),
    "merge": (
        "36f4915a83458c73f12ff364ebf4ccdd45e6b826fe305e52429098967d26bb55"
    ),
    "source_credits": (
        "f3c9f0c68484f0685560f896d3f782c5c75eb2edf336184ba3ca230bddced095"
    ),
    "extend": (
        "c0d7b44202b40992e33ae23c530fa6c135ff003e23765dde461a044a3a6d8d46"
    ),
}
_PRODUCTION_P2P_SENDER_FRAME_ITEM_SHA256 = {
    "encrypted_frame_geometry": (
        "fa320e44681f1928dd862e8f07b1ea3c72b2c409ba58bf88556c2edd6d57b929"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_encoded_buffer": (
        "eb243e718f0b02455dcb31f009ea22aca2f33b4c27aaa00cd309846d87a159fb"
    ),
    "check_queue_limit": (
        "097b47794fe8e9d4ded5357204dfe755f99d476323e11d4c50b604efad0dfb48"
    ),
    "account_enqueued": (
        "dc14a3102e45e8487ed71d3cf7c43af270be5bad47b00fe69fac9f85e51dba13"
    ),
    "enqueue_encrypted": (
        "1efc3f5c8f677f117d82e7e1dcec9c489fbddbcf970b3bba74fd184c5f138421"
    ),
}
_PRODUCTION_P2P_START_FRAME_ITEM_SHA256 = {
    "validate_encrypted_frame_cap": (
        "5305bae9d0febfc2a1348f8f3b9737fb5155a62084a80f731d75bc372ea3bbcd"
    ),
    "start_with_crypto": (
        "2b55c0b771acdf3de90b16e7baabf4598d140a95f3467f3fb16d6fb94d1b626b"
    ),
}
_PRODUCTION_P2P_RELIABLE_PEER_ITEM_SHA256 = {
    "post_recover_with_flush_ack": (
        "efe28549106501cb539ea51d42e5c318f5c54bfeb8bef94e1cd1a78a85ea499a"
    ),
    "post_recover_inner": (
        "004f480809bdd5e6f456a5da08b2acd192fb817c44606eec5066141aed2f5b7f"
    ),
    "acknowledge_flush": (
        "b2f9e0921009c50799677284aef793d9a76d184db88b24b1b242eb093e8f71cb"
    ),
    "prepare_owned_or_defer": (
        "0fab3dc65badad08481063c4517f590710ea0592f6e3094d8df05df6a8872f50"
    ),
    "prepare_message_with_ownership": (
        "2d6ef5bd05a9b9ac31e254546f0e10509df5211daae262d8daa38de8049aa441"
    ),
    "prepare_or_defer_with_ownership": (
        "271a4549dc2075166aa9c647a6477f1748f32b536d790dcef967ca9c32087a4d"
    ),
    "retry_deferred": (
        "37298defcaba75e4f3b1e32b55cc6025629e350e2c1a3efe3b358aaad719b3d9"
    ),
    "flush_plain_high": (
        "38da1a9853b5a4c780a3c4dc898ec9ba328b39c1128b7fce7cd2922d2ed5c32b"
    ),
    "flush_plain_low": (
        "bf7e0c4580f6a886ed26d61258f5d8017821b9ab8246ceed1e1f63e510676dd6"
    ),
    "enqueue_current_buffer": (
        "e105440c89db28ab02b220b287af24f04939d31ef0e26bde32cc0346f0450a69"
    ),
    "acknowledge_flushed_batch": (
        "30d441537005fc905493103db122d17670def105f67d36993341038b674a4c63"
    ),
    "fill_batch": (
        "30bba0f4890fd277ee9b22cd433178d840418dbd22a1a615f2a1153857b09ae7"
    ),
    "pop_high_frame": (
        "e7bd48c39d938732bd44cff707989aca7e8d1f55e75c20d559a35ad788302a74"
    ),
    "pop_low_frame": (
        "3d472752bbb83b40712993959290bacdbebe26d604c38500f701642ea62b33a9"
    ),
    "send": (
        "a10e589b0e785b9d113d693e7946a35bc1b6dabb7f8f72c59ff713efe3c393e5"
    ),
    "send_one_ready_stream": (
        "aa54b3d9df965c06ab3b66175859fa31ef5fd377c1bbeacc049a9d8d69a1c7c3"
    ),
    "next_peer_stream_io": (
        "178e60084380d1ac3b9abf5cdcc866748d8de270d215846f85dcc63a7c3b9539"
    ),
    "run": "4a0859c7941e2adcb36420b9cfba9e4746e4b8065f38235fdc24b3f19373272e",
    "reattach_reply_route": (
        "120803740de09553bb9112a556cceed7e2db414f4f5da3a9691a7886b5264be0"
    ),
}
_PRODUCTION_P2P_RELIABLE_PEER_QUALIFIED_ITEM_SHA256 = {
    "OutboundPostOwnership::new": (
        "3fc9b02c402019b5eed09485217d42cdeaabe5bf471e0786f0706ac2ba1af503"
    ),
    "RetainedPost::new": (
        "cbb4bebe6f001066fd4491bd693776fc007a932fc524ffed5de1c92209b561a4"
    ),
    "RetainedPost::into_parts": (
        "91b64ad394e845b083fe498e683d447e6d27c5e15cfbf0aa7853fb9633f45769"
    ),
}
_PRODUCTION_P2P_RELIABLE_NETWORK_ITEM_SHA256 = {
    "new": (
        "b06baebfca688fe1182cab468451157e75320543600ec679cfdd46a79b420f61"
    ),
    "validate_delivery_binding": (
        "f231d50a1984538606124ca88b6bfda2be639cc399e2592bf69c43e7bbc10337"
    ),
    "reliable_progress_class": (
        "542367e1dbca6344eab5fd6d22d8614ad543ee29479dae51fea97963eccc75ae"
    ),
    "is_reliable_progress_route": (
        "513e759a4dee7cd1367971b3d7ebafd462134d609421a5ac96da1cfb3f1f3bfb"
    ),
    "for_route": (
        "9163a97125d966988627b262bfaff254eb55239a9957937b395f52cc580cd516"
    ),
    "semantic_target": (
        "7312c96797e37a2a05381feee6f92a4f127b95b1e31c411a1dc517123e8e789f"
    ),
    "source_key": (
        "555b18d0375cc9268d427b6fb04e05b1d5a48099d8bb48921ea85ea1c19e77b8"
    ),
    "authenticated_via": (
        "3e66e6dd0a33ffb60c71db2a4f306abe6c733d54558b94559664503576695de2"
    ),
    "same_tenure": (
        "6119490d174355c24153db559dc66834d10bee4f81c3dbcbaab9fb460cbb24a5"
    ),
    "same_delivery": (
        "dbab8b1b889a1b4ab4e1b8bac73fe981b38eaf943c0f4ed4f5c259bdacec2160"
    ),
    "equal_ordinal_different_tenure": (
        "22b9f323b4a2ff799b1be14ad0f52b3e30633167151e7ce40a2a1f9cef8363ab"
    ),
    "equal_connection_ordinal_different_tenure": (
        "926e428abdd009f73e23f5569410bf36ab1947fb65eca3dc22225751b912a2b6"
    ),
    "same_source": (
        "b95727867096f1decdc53b785e1414412643e70259488585a5571c5cabbcd573"
    ),
    "source_update_from": (
        "e468f9e4d47b950d751340588efbf3dc18def32cc39eedc823c354a0f531ac8a"
    ),
    "source_update_from_snapshot": (
        "8accccfafb03f27d697224fdfde5a2caa2ec6d06746b8c81cdcb07bd8d44ef24"
    ),
    "source_freshness_from": (
        "de335eea19992a5dd1157b00bb2c8ee9bef7067a6626e2ab2b95a96b1f7d0e37"
    ),
    "is_reply_writable": (
        "d53b0fcdbaa42d480a2ebc08b9fe0fba542be5e7ffcd901c0e7564a7553cc97e"
    ),
    "same_request_authority": (
        "96bbd9cc1360fea28d3d6caa772fb5ead105d0bd1fc44466fd977882cac87be5"
    ),
    "try_from_route": (
        "8766ab36eb93485a362dff5c943c7041b2db370f2e5a6a10254c3ad05c333285"
    ),
    "source_capacity": (
        "8a7caf36391c3d7a43862cbfb3de6dcc23b56553321cc34cbfc3b9b383a19557"
    ),
    "retain_active": (
        "e0e6ea9560f49fc055481959c24ec1aeea0918dd09110bc293954f93452bda85"
    ),
    "retain_active_with_receipt": (
        "50df0a9f34c9a05268b4a723e8813fc5fb09259627a069a6f3056c160ec5b27d"
    ),
    "retain_active_with_receipt_after_snapshot": (
        "9f682998f17416d9dd3adfb73a67fda005abf3ca9b0013ec3f3c63e63c2a8e45"
    ),
    "merge": (
        "8b850f96609375104f69852bca87796e62fb3038c2a2e2ca53330114d6a42e33"
    ),
    "merge_with_receipt": (
        "b5e6d03122445a97b51b13d127ca4ea51a7728df5db54bec518aa8e1136043f2"
    ),
    "merge_observed_with_receipt": (
        "7fde28579ed7717942a7ae9ee7cb680e3d5cca8a772b6b9f547e263055bd96fe"
    ),
    "same_exact_history": (
        "cf777335ed65929280f27fbb395af144781f33875cc5d9e2ea3027cf011a46f1"
    ),
    "has_valid_container_shape": (
        "eb733983ed3ea40244c5d6033290582cb65f5b3e7cb827ecc9bdc5d2742f2f38"
    ),
    "preflight_merge": (
        "b4577057f91f3710657d828e83b59ea09f90e41f6ee6965754c05cfd38bba401"
    ),
    "attach": (
        "0c357d084de3e1dddfecf9a291afcd91b96c126d9a8f1a25922478b146eb5903"
    ),
    "validate_after_retired_delivery": (
        "17aa33a5ba1cae849d7eba47dba76327ac85e6a163c5a14b82517a653ccbf6a9"
    ),
    "merge_retired_delivery": (
        "e211a40c5abb7c3438caa11d0838ac061ca0ea339d0c13f150b431320b18e4cc"
    ),
    "record_retired_delivery": (
        "7a331e89f673fa55472e39a2343881b8754b7612cd22d07817056805e34cf513"
    ),
    "release_retired_tenure_binding": (
        "fcb910d728e26b11a0f555edd7aa7fe85b16bf7617ef4951670e90f9cf294446"
    ),
    "reply_route_source_capacity": (
        "6501b55351f33bd6fe59db3c97c28bd69c586e8e9ff94eab09e1c30876923fa7"
    ),
    "peer_connected": (
        "43f713676440d7756bcadf7729ea2432c30262b8c11cbb751dac8657e9423445"
    ),
    "peer_message": (
        "fc267abe5dfc2417a9d51978ac73cd5c10912b7a2f11593491100c8f203f4485"
    ),
    "progress_ticket_request_digest": (
        "ab51b06be057b794221217b6505e2cd4abbb66c2d7f87504a6f2257c260124c4"
    ),
    "matches": (
        "d7aa1ff1bba2b408eacaa4821e5c8791a3298f8ecbbc10a120e4cf4f18dd00aa"
    ),
    "try_reserve_for_source": (
        "b938440d9f487ac064ca8ac46269d88551da51349ce14b4e470c87d672e82888"
    ),
    "submit_progress_message_to_source": (
        "78d5131df35f3d0972597912413d118efcb28be723116c3e29ffeaf9500ba546"
    ),
    "new_targeted_post": (
        "cfaedf859e6596e63c2806c44afcca063f66ec1e660f75047e433ae5a30e727f"
    ),
    "into_parts": (
        "18df4a3deca18212833fd5179f14b15c77399db9c31374ff1afe4486e93c4982"
    ),
    "push_back": (
        "62083b1dfc8bb3d33a14cdcd5ceb441750ee4fe0ce95477bd01e8a985748eadd"
    ),
    "pop_front": (
        "81ec2f9581812789d6bca5ed2a1c608b7c9f4cda17604c3ebdcaa2c3c7d3cfd0"
    ),
    "retry_back": (
        "8c2994b9ac91ebce8e4d28bf394c902655f2f64a5bcc3547b050ac49fd92c38f"
    ),
    "defer_high_priority_network_message": (
        "1b8948a28bf0ccae5431b18c25abd9507b2b832433306f28af50565e960e2dda"
    ),
    "post_reply_recoverable": (
        "e81939836efa2f08aea3d4cc74424e1c15a17dcd25c3718290c90f68ffa2f463"
    ),
    "post_reply_recoverable_with_flush_ack": (
        "b7b67e256375e22c86ff57fb7ccbb26131b24f0b191a5ed317cab1dcce89b888"
    ),
    "post_reply_recoverable_with_flush_ack_inner": (
        "b3597f519695465158e058cd91b48617aaace25fa496a5f9f64833c010c84c8c"
    ),
    "broadcast_recoverable": (
        "548d457392659fd2c68e3d27ea3796ad8477a2e3afd7f93b9930c49d76a66f96"
    ),
    "into_dispatch_parts": (
        "cca1a2804e76c9ba9d07319bf5256be6c53c2e0c39237ace52aa53b86a75a5c2"
    ),
    "retain_after_dispatch_attempt": (
        "6dfa7ebe62b1a848b55c0ef43f945296b51f9014dfff44a1166bf042a6ca582b"
    ),
    "cancel_reply_route": (
        "9bb0a32ff652a06ef803fb010f0ca3ad5a44cca6a16b047ed77da7eb8a1307d7"
    ),
    "cancel_authority_waiters": (
        "cf554e96f17a00ee8f018d6826d8be8efd0366ff7e0c256349353d9f2df3c39e"
    ),
    "release_cancelled_targets": (
        "88ad8d2a8a0183ec28c8baf59d40af851b135abd7c46af65ed411f9ef7fe5b4d"
    ),
    "dispatch_reliable_actor_message": (
        "21cf34eb5aa68209a6baf29124170f073ea3a65be8c4e5887d6c5fbb69ffe20f"
    ),
    "dispatch_reliable_actor_message_inner": (
        "3988f0adb7eae269376964cbe9bd11e6437b7c9955076523704ee7f9f6633687"
    ),
    "post_reliable_actor_frame_to_writer": (
        "9347732994ea2854fc8185768ca08bbd0b3bac410fdb65b3d724b4ccb71d0c4b"
    ),
    "retry_reliable_actor_messages": (
        "5d59dda16998eea883d6c96f6fedde6fa3f8e35a82f16fff6849d35cd4f3ce7c"
    ),
    "accept_reliable_actor_message": (
        "93884415a18ff4e3f7c0c680d0219b8a1d01ebdca398b9c8534aa678a20a21e0"
    ),
    "mark_connection_terminating": (
        "fdba9b05c60f8a85f3a2c03e0b98959ea0e78f143a17e3fc7a2af6000c1f4238"
    ),
    "finish_reply_route_tenure": (
        "50b38fb6bb32cfab9664e611b559cf93e8a9bf566f13883c7acfabbd1c8b0c1d"
    ),
    "handle_service_message": (
        "e148a564c7f53e1bac76b5ba3c0f75070281db95a64e4e7ed2dbb3f5233d85df"
    ),
    "peer_terminated": (
        "32c662f6a4e5be0b27d9fc7510076bc203b9a5dd7782b22796e89eeeb89fcd9c"
    ),
    "cancel_all_reply_route_tenures": (
        "85556605246c148cb390b143406a6a5320fc14c57ec964c52b1eb5f0f9395ca1"
    ),
    "drop": (
        "ecd2987b63057f26e35d5753bf94c5c68fc4d720c9529630c4b1ff52e18b69ce"
    ),
    "run": (
        "ba3b7813c98ba070898a27489f71abc52bb8a08744c6053826912574ea34f05c"
    ),
}

_PRODUCTION_P2P_REPLY_FLUSH_ACK_ITEM_SHA256 = {
    "new": "58bdb00f6634be8d93048d75953b45a4ff2354798f99aff9b2f58c03562f04cf",
    "poll": "512c90e2877329331a6ec24ae26eb1a6d021fe735cd42756def239d1e5101cd2",
}

_PRODUCTION_P2P_PROGRESS_LEASE_DROP_SHA256 = (
    "5786fb4d2c21a18923cc7b636dd6a5634f91c6d47879958977eadde37c419f07"
)
_PRODUCTION_P2P_REPLY_ROUTE_IS_ACTIVE_SHA256 = (
    "e2e80f75efe8739fab1d6030ffc1cd0af070ee6d223eca06b953d81b2c3537a7"
)
_PRODUCTION_P2P_REPLY_ROUTE_IS_AUTHENTICATED_VIA_SHA256 = (
    "817add7dcd219464c6d20df132fbd8bfcfe10a0caf56f5a815d86e975b3d03af"
)
_PRODUCTION_P2P_REPLY_ROUTES_MERGE_OBSERVED_SHA256 = (
    "9d21e1b0114d35991682d2f5f16072d33ffb9439426f551a6fe68183120d3e3f"
)
_PRODUCTION_TRANSPORT_REPLY_ROUTE_ITEM_SHA256 = {
    "try_from_transport_with_reply_route": (
        "614facaea868b070cd9ce0dc08e8bb472f3afc3fff9769f09a85128e54a7c11e"
    ),
    "transport_reply_route_construction_is_fallible_and_target_bound": (
        "4a26c079cad7e57615605b7af734211e3b62ba1e9dc54c36f2e0581fe5c23190"
    ),
}
_PRODUCTION_DAEMON_FRAME_VALIDATION_ITEM_SHA256 = {
    "validate_config": (
        "39a6c98e410ac1a5ddf8af00fbacf8b1a49c167228bddaacf770f4cca616762c"
    ),
    "validate_config_offline": (
        "7cb189cf3f5ac23ca68098a57c655e7e8808642de1c90d76a4270603b5426ed2"
    ),
    "validate_network_frame_runtime_limit": (
        "551d207e8371c5966274fc759d379f75f281127f668c6555aba347c385066b92"
    ),
}
_PRODUCTION_P2P_CAP_ITEM_SHA256 = {
    "frame_plaintext_cap": (
        "d09e8a86d8c9d6d4c3e8a9c9f202debab61e11f3256f2e1361d3895a0dd97b33"
    ),
    "frame_queue_charge": (
        "995d225539dcc72b840fc6c807cf380926b4d552da2da05b022de0eb20f85b5d"
    ),
}
_PRODUCTION_SM_DISTID_GEOMETRY_ITEM_SHA256 = {
    "validate_distid": (
        "205a20a45faa4455d25b8e9d2501f6ab66a2b069a83b8e358a9645d71c94181d"
    ),
}

# Exact pure geometry and admission items connecting the abstract 4N+2H+2
# owner model to the authoritative production queue.  Whole-item token seals are
# intentional here: every numeric term contributes to a progress reservation,
# and comments or test-only lookalikes cannot satisfy this contract.
_PRODUCTION_FAIR_V2_INGRESS_TOP_LEVEL_ITEM_SHA256 = {
    "fair_v2_ingress_is_certified_body_request": (
        "749f33ce31fcfe4ecf84e2264c181b909d30043bb759a90cbf6e5ebb1a40d0e0"
    ),
    "fair_v2_ingress_required_capacity": (
        "838df8ad9c809753541a1ee4f75390a763fd4f3362231407626e80102acad249"
    ),
    "fair_v2_ingress_current_protected_slots": (
        "277b32d5a3f4564a998edee5a0267204553f659134fce846de04353bd0ba34d4"
    ),
    "fair_v2_ingress_lane_protected_slots": (
        "da25270d56a0bd3011ad224033c9b4a5c7a42ca4e86e66f1eb6ca1018415b9ea"
    ),
    "fair_v2_ingress_required_byte_capacity": (
        "f3b4cfc1017778a7d7ba68b1e4c3a24c2f00f5f20252d3375eded1e986bd0799"
    ),
    "fair_v2_ingress_compact_len_prefix_bytes": (
        "50cd13b1d620e26eb0502ae9650b7cb66e489073ab407d95a5217177de517d95"
    ),
    "fair_v2_ingress_framed_bytes": (
        "d311f9815d146c9cc4539653a399152e0b3cf32f3d47060d9af23aa19c04e7c5"
    ),
    "fair_v2_ingress_required_manifest_bytes": (
        "5904b0cb28ad048ab778e75e1f99bc19336b6712d817d33ac7a07e050f25c7e3"
    ),
    "fair_v2_ingress_required_quorum_certificate_bytes": (
        "0335a87586cee9c0d8bf68ba87581f873ccbfdce5c316b1e033ccf0019801629"
    ),
    "fair_v2_ingress_required_proposal_bytes": (
        "f87150d121741f99778f8108a82cc95a0811afe51edae3cf06ee8e955337985b"
    ),
    "fair_v2_ingress_network_message_bytes_from_block_message": (
        "c92a1c58b6296fd3afb040497b3f971ec509ffa2b5408f7c0cb48c5ec0a5b979"
    ),
    "fair_v2_ingress_network_message_bytes": (
        "5e605df8dc71ee5961cf2c40b3dc8c6108c1c745662a5249d533b1dc8c9fd6eb"
    ),
    "fair_v2_ingress_required_p2p_frame_bytes": (
        "a72706f96788cbab7cb43997ec1dc97e8168b2e5dc8e5d1817db641179f8d7ae"
    ),
    "fair_v2_ingress_required_lane_p2p_frame_bytes": (
        "db83412d500e2f53d7959462f230a02afe85cfa87d1ebf05dd1744a1aacf88d6"
    ),
    "fair_v2_ingress_v2_envelope_bytes": (
        "6a5e066c134eb45a8b64fb5e3b62aab6527ef05f4f6c4459666d56929f13723e"
    ),
    "fair_v2_ingress_embedded_peer_id_bytes": (
        "6a511e21a8e16c6b4db002e3276a5d98d9797249a2d1c3b7549931d6bf24f5bb"
    ),
    "fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key": (
        "8456504122db0dfe12337fce4f14d0e107a3802f990bcf3b9f28dc946ee67b8c"
    ),
    "fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes": (
        "67264f5f5699090643b5ba1554f750f89ade5b6f8e36e12f6d10c15607783b9f"
    ),
    "fair_v2_ingress_required_block_sync_p2p_frame_bytes": (
        "dbbe6e8853781b0d42843c59a4028d103a938493d45d5f5e5551356b884a6aec"
    ),
    "fair_v2_ingress_required_recovery_request_bytes_for_key": (
        "5f0753ed526343afa6f7db6c44b2c6fdeeaa9a036ca9fbd0aeb64d1b3da5437c"
    ),
    "fair_v2_ingress_required_recovery_request_bytes": (
        "56734857457d1be558c8a3e79848cac1dc6745265be2647115ea8fcf818630d9"
    ),
    "fair_v2_ingress_required_commit_certificate_response_bytes_for_key": (
        "f231a220131a213c5bdaae922c075e2f00bae7a39cbfba408ed86149ba42cf14"
    ),
    "fair_v2_ingress_required_commit_certificate_response_bytes": (
        "a799e86fd78987da5fea276ffbda96d6078e628e3943c1edb223aa60c89fb81c"
    ),
    "fair_v2_ingress_required_transport_completion_bytes": (
        "a8816bf106a1a62a8b8c411f5964aeeef1ec14f86d14f2313b5cae353e1b359c"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_CLASS_ITEM_SHA256 = {
    "classify": (
        "4c5af83b512d633256649e19265a22e80ef9d2e5fde50507ec91c075642b98e6"
    ),
}
_PRODUCTION_FAIR_V2_INGRESS_IMPL_ITEM_SHA256 = {
    "new_with_source_geometry_and_transport_frame_caps": (
        "b8d9a480d1cd0e582576891f2af20370ef35f5af4dcdc6c9fe24eafc6e30f3c1"
    ),
    "configure_roster_for_context": (
        "13d8e7a240e22e620a6627a6f34c5be856eb22cb9e2f8a38bd2b3e3f95b21334"
    ),
    "configure_roster_with_byte_requirements": (
        "193a8e4586cc6ee7689c9cc15e45474c129a217e91be760559027fd1939f9d02"
    ),
    "open": (
        "d19623cdc7ad56922f91c7a9c0d2513199ecafa4e6ad26ea2717073d561b41eb"
    ),
    "try_push_at": (
        "83c9e853b1bab4a43df359ead4dab381471dc1c503edc1c61e5bc1a39155947f"
    ),
    "try_recv_if_at_checked": (
        "24f0b8b8678c7ca64f3f46b2d214553be166e6a2bb2d8ec21f3de8968a35e7ef"
    ),
}
