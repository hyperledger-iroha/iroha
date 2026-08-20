"""Late-bound proof-ledger cases executed in the canonical test namespace."""


def wrap_tla_theorem_proof_step(
    source: str,
    symbol: str,
    anchor: str,
) -> str:
    """Wrap one anchored structured proof step in an invalid temporal box."""

    declaration = re.search(
        rf"(?m)^THEOREM\s+{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
        source,
    )
    assert declaration is not None, symbol
    next_declaration = re.search(
        r"(?m)^(?:(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|={4,}\s*$)",
        source[declaration.end() :],
    )
    theorem_end = (
        len(source)
        if next_declaration is None
        else declaration.end() + next_declaration.start()
    )
    theorem = source[declaration.end() : theorem_end]
    assert theorem.count(anchor) == 1, (symbol, anchor)
    anchor_offset = theorem.index(anchor)
    labels = [
        match
        for match in re.finditer(r"(?m)^[ \t]*<\d+>\d+\.[ \t]*", theorem)
        if match.end() <= anchor_offset
    ]
    assert labels, (symbol, anchor)
    label = labels[-1]
    proof_marker = re.search(
        r"(?m)^[ \t]*BY\b",
        theorem[label.end() :],
    )
    assert proof_marker is not None, (symbol, anchor)
    step_end = label.end() + proof_marker.start()
    assert anchor_offset < step_end, (symbol, anchor)
    step = theorem[label.end() : step_end]
    formula = step.rstrip()
    trailing = step[len(formula) :]
    mutated_theorem = (
        theorem[: label.end()]
        + "[]("
        + formula
        + ")"
        + trailing
        + theorem[step_end:]
    )
    return (
        source[: declaration.end()]
        + mutated_theorem
        + source[theorem_end:]
    )


def test_rust_item_scanner_ignores_lint_only_inner_cfg_attr() -> None:
    """A conditional lint annotation must not masquerade as compile gating."""

    module = load_checker()
    source = (
        "#![cfg_attr(not(test), allow(dead_code))]\n"
        "pub fn always_compiled() {}\n"
    )
    (item,) = module.rust_items(source, "always_compiled")

    assert item.ancestor_inner_attributes == ()


def copy_serviced_candidate_production_fixture(tmp_path: Path) -> None:
    """Copy the durable candidate store and its adapter integration."""

    for relative in (
        Path("crates/iroha_core/src/sumeragi/mod.rs"),
        Path("crates/iroha_core/src/sumeragi/safety_wal.rs"),
        Path("crates/iroha_core/src/sumeragi/serviced_candidate_store.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)


def copy_timeout_vote_episode_fixture(tmp_path: Path, module) -> Path:
    """Copy only the Rust and TLA+ sources bound by the timeout episode."""

    for relative in (
        Path("crates/iroha_core/src/sumeragi/mod.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("formal/sumeragi_v2/SumeragiV2AsyncNetwork.tla"),
        Path(
            "formal/sumeragi_v2/"
            "SumeragiV2AsyncRecoveryVoteEpochProofs.tla"
        ),
        Path(
            "formal/sumeragi_v2/"
            "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla"
        ),
        Path(
            "formal/sumeragi_v2/"
            "SumeragiV2AdequateLeaderServiceClosureProofs.tla"
        ),
        Path(
            "formal/sumeragi_v2/"
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla"
        ),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(module.ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)
    return tmp_path / "formal" / "sumeragi_v2"


def test_timeout_vote_episode_source_fidelity_rejects_missing_reviewed_include_component(
    tmp_path: Path,
) -> None:
    """The timeout episode fails closed when a reviewed Rust include is absent."""

    module = load_checker()
    formal_dir = copy_timeout_vote_episode_fixture(tmp_path, module)
    missing_relative = Path(
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs"
    )
    missing_component = tmp_path / missing_relative
    canonical_component = ROOT_DIR / missing_relative
    assert missing_component.is_file() and not missing_component.is_symlink()
    assert missing_component.read_bytes() == canonical_component.read_bytes()
    missing_component.unlink()
    missing_errors = module._timeout_vote_episode_source_fidelity_errors(
        tmp_path, formal_dir
    )
    runner_parent = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    assert (
        f"{missing_component}: reviewed Rust include component for "
        f"{runner_parent} must be a regular non-symlink file"
        in missing_errors
    ), missing_errors


def test_timeout_vote_episode_selector_preserves_strict_before_dependency_after_digest_refresh(
    tmp_path: Path,
) -> None:
    """The shared selector cannot service dependencies before ordinary ingress."""

    module = load_checker()
    formal_dir = copy_timeout_vote_episode_fixture(tmp_path, module)
    assert (
        module._timeout_vote_episode_source_fidelity_errors(
            tmp_path, formal_dir
        )
        == []
    )
    relative = Path("crates/iroha_core/src/sumeragi/mod.rs")
    item_name = "select_fair_v2_ingress_candidate"
    mutate_rust_item_source(
        module,
        reviewed_rust_item_provider(module, tmp_path, relative, item_name),
        item_name,
        "for dependency_pass in [false, true]",
        "for dependency_pass in [true, false]",
    )
    rebind_timeout_vote_episode_rust_item_seal(
        module,
        tmp_path,
        relative,
        item_name,
    )
    errors = module._timeout_vote_episode_source_fidelity_errors(
        tmp_path, formal_dir
    )
    assert any(
        "shared TimeoutVote selector must preserve strict-before-dependency, "
        "Blocked exclusion, downstream predicate, and exact disposition"
        in error
        for error in errors
    ), errors


def rebind_timeout_vote_episode_rust_item_seal(
    module,
    repo_root: Path,
    relative: Path,
    item_name: str,
) -> None:
    """Rebind only the deliberately mutated timeout-episode Rust item."""

    relative = Path(relative)
    path = repo_root / relative
    items = module.rust_items(path.read_text(encoding="utf-8"), item_name)
    assert len(items) == 1, (relative, item_name)
    digest = module._rust_item_token_sha256(items[0])
    rebound: list[str] = []
    role_relatives = {
        "ingress": Path("crates/iroha_core/src/sumeragi/mod.rs"),
        "runner": Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        "lifecycle_runner": Path(
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"
        ),
        "pending_runner": Path(
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
        ),
        "runtime": Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
    }
    for key in module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256:
        role, qualified_name = key.split("::", 1)
        if (
            role_relatives[role] == relative
            and qualified_name.rsplit("::", 1)[-1] == item_name
        ):
            module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256[key] = digest
            rebound.append(key)

    for seals, group_relative in (
        (
            module._TIMEOUT_VOTE_EPISODE_RUNTIME_REGRESSION_SHA256,
            Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        ),
        (
            module._TIMEOUT_VOTE_EPISODE_INGRESS_REGRESSION_SHA256,
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
        ),
        (
            module._TIMEOUT_VOTE_EPISODE_WORKER_REGRESSION_SHA256,
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        ),
    ):
        if group_relative == relative and item_name in seals:
            seals[item_name] = digest
            rebound.append(item_name)
    assert rebound, (relative, item_name)


def rebind_timeout_vote_episode_tla_operator_seal(
    module,
    formal_dir: Path,
    filename: str,
    symbol: str,
) -> None:
    """Rebind only the deliberately mutated timeout-episode operator."""

    seals = module._TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256[filename]
    assert symbol in seals, (filename, symbol)
    source = (formal_dir / filename).read_text(encoding="utf-8")
    extracted = module._top_level_operator_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None, symbol
    body, _ = extracted
    seals[symbol] = hashlib.sha256(
        " ".join(body.split()).encode("utf-8")
    ).hexdigest()


def rebind_timeout_vote_episode_tla_theorem_seal(
    module,
    formal_dir: Path,
    filename: str,
    symbol: str,
) -> None:
    """Rebind only the deliberately mutated timeout-episode theorem."""

    seals = module._TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256[filename]
    assert symbol in seals, (filename, symbol)
    source = (formal_dir / filename).read_text(encoding="utf-8")
    extracted = module._top_level_theorem_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None, symbol
    body, _ = extracted
    seals[symbol] = hashlib.sha256(
        " ".join(body.split()).encode("utf-8")
    ).hexdigest()


def copy_async_source_fidelity_fixture(
    tmp_path: Path, module, *formal_names: str
) -> Path:
    """Copy the async formal inputs and their production-source bindings."""

    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/mod.rs"),
        Path("crates/iroha_core/src/sumeragi/serviced_candidate_store.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
        Path("crates/iroha_kagami/src/localnet.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
        Path("crates/iroha_crypto/src/lib.rs"),
        Path("crates/iroha_crypto/src/sm.rs"),
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_p2p/src/lib.rs"),
        Path("crates/iroha_p2p/src/network.rs"),
        Path("crates/iroha_p2p/src/peer.rs"),
        Path("crates/irohad/src/main.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/render_taira_validator_bundle.py"),
        Path("scripts/verify_sumeragi_v2.sh"),
        Path("xtask/src/kagami_profiles.rs"),
        Path("defaults/kagami/iroha3-taira/config.toml"),
        Path("defaults/kagami/iroha3-taira/genesis.json"),
        Path("configs/soranexus/taira/config.toml"),
        Path("configs/soranexus/taira/genesis.json"),
        Path("configs/soranexus/taira/README.md"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)
    for name in dict.fromkeys(
        (
            *formal_names,
            "SumeragiV2AsyncTemporalClosureProofs.tla",
            "SumeragiV2AsyncStage6Proofs.tla",
            "SumeragiV2AsyncRecoveryVoteEpochProofs.tla",
            "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs.tla",
            "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
        )
    ):
        destination = formal_dir / name
        if name == "SumeragiV2AsyncLivenessProofs.tla":
            destination.write_text(
                module._async_liveness_source(module.FORMAL_DIR),
                encoding="utf-8",
            )
        else:
            shutil.copyfile(module.FORMAL_DIR / name, destination)
    return formal_dir


def rebind_reviewed_rust_item_digests(
    module,
    source_path: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    bindings: tuple[tuple[dict[str, str], str], ...],
) -> tuple[tuple[dict[str, str], str, str], ...]:
    """Rebind selected seals so a mutation must trip a semantic contract."""

    items = tuple(
        item
        for item in module.rust_items(
            source_path.read_text(encoding="utf-8"), item_name
        )
        if item.brace_context == context
    )
    assert len(items) == 1, (source_path, item_name, context)
    digest = module._rust_item_token_sha256(items[0])
    original = tuple((table, key, table[key]) for table, key in bindings)
    for table, key in bindings:
        table[key] = digest
    return original


def restore_reviewed_rust_item_digests(
    original: tuple[tuple[dict[str, str], str, str], ...],
) -> None:
    """Restore seals rebound by one digest-resilient semantic mutation."""

    for table, key, digest in original:
        table[key] = digest


def assert_digest_independent_consume_effect_order_mutation(
    module,
    formal_dir: Path,
    effects_path: Path,
    mutate_effect_item,
) -> None:
    """Reject replay-plan/retention reordering after refreshing the item seal."""

    canonical_effects = effects_path.read_text(encoding="utf-8")
    effects_path.write_text(
        mutate_effect_item(
            "consume_effects",
            "        let local_proposal_replay_projections = self\n"
            "            .plan_local_proposal_replay_consumptions(&effects, &ownership)\n"
            "            .map_err(|error| self.close(error, services))?;\n"
            "        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {\n"
            "            return Err(self.close(error, services));\n"
            "        }",
            "        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {\n"
            "            return Err(self.close(error, services));\n"
            "        }\n"
            "        let local_proposal_replay_projections = self\n"
            "            .plan_local_proposal_replay_consumptions(&effects, &ownership)\n"
            "            .map_err(|error| self.close(error, services))?;",
        ),
        encoding="utf-8",
    )
    original = rebind_reviewed_rust_item_digests(
        module,
        effects_path,
        "consume_effects",
        (("impl", "<", "R", ":", "EffectRuntime", ">", "V2EffectExecutor", "<", "R", ">"),),
        ((module._PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256, "consume_effects"),),
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "plan local replay, retain the complete batch, transfer each preflighted replay authority"
        in error
        for error in errors
    ), errors
    assert not any(
        "retained effect FIFO consume_effects declaration" in error
        for error in errors
    ), errors
    restore_reviewed_rust_item_digests(original)
    effects_path.write_text(canonical_effects, encoding="utf-8")


def assert_digest_independent_drive_effect_order_mutation(
    module,
    formal_dir: Path,
    adapter_path: Path,
    mutate_drive,
) -> None:
    """Reject WAL-owner/event reordering after refreshing both item seals."""

    canonical_adapter = adapter_path.read_text(encoding="utf-8")
    adapter_path.write_text(
        mutate_drive(
            "                    self.pending_persistence_id = None;\n"
            "                    let persisted = reducer::Event::Persisted { tag, id };",
            "                    let persisted = reducer::Event::Persisted { tag, id };\n"
            "                    self.pending_persistence_id = None;",
        ),
        encoding="utf-8",
    )
    original = rebind_reviewed_rust_item_digests(
        module,
        adapter_path,
        "drive_effects",
        (("impl", "SumeragiV2Adapter"),),
        (
            (module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256, "drive_effects"),
            (module._SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256, "drive_effects"),
        ),
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects must validate the exact appended WAL receipt and retained payload"
        in error
        for error in errors
    ), errors
    assert not any(
        "drive_effects declaration, contract, and complete control flow must match"
        in error
        for error in errors
    ), errors
    restore_reviewed_rust_item_digests(original)
    adapter_path.write_text(canonical_adapter, encoding="utf-8")


_FOLDED_RUNTIME_CANDIDATE_MUTATIONS = (
    (
        "effect_candidate_semantic_binding",
        _RUNTIME_DRIVER_IMPL,
        "production_adapter_effect_candidate_binding(effect, effective_inherited.as_ref())",
        "production_adapter_effect_candidate_binding(effect, inherited)",
        "production RuntimeDriver must route every candidate through the typed inheritance and refinement gate",
    ),
    (
        "effect_candidate_semantic_binding",
        _RUNTIME_DRIVER_IMPL,
        "if *round == proposal_round && *subject == decision_subject =>",
        "if *round == proposal_round || *subject == decision_subject =>",
        "durable Decision body recovery must reconstruct Commit authority only for the exact proposal round and subject",
    ),
    (
        "effect_candidate_semantic_binding",
        _RUNTIME_DRIVER_IMPL,
        "parent.commit_refinement_to(decision_statement).is_none()",
        "parent.commit_refinement_to(decision_statement).is_some()",
        "durable Decision body recovery must reject an incompatible inherited authority before publication",
    ),
    (
        "effect_candidate_semantic_binding",
        _RUNTIME_DRIVER_IMPL,
        "Some(_) | None => inherited.copied(),",
        "Some(_) | None => None,",
        "nonmatching or absent durable Decision recovery must preserve only the inherited authority",
    ),
    (
        "body_pipeline_completion_is_owned_by",
        _RUNTIME_PRODUCTION_SERIALIZED_IMPL,
        "let result = (plan.retained_owner.clone(), plan.effective_statement());",
        "let result = (ownership.owner().clone(), plan.effective_statement());",
        "in-flight body completion coalescence must resolve, refine, and return its one exact incumbent owner",
    ),
    (
        "enqueue_body_pipeline_completion_with_owner",
        _RUNTIME_PRODUCTION_SERIALIZED_IMPL,
        """self
            .body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
            .is_some()""",
        """self
            .body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?
            .is_none()""",
        "owned body-completion retries must compare the incumbent before queue coalescence",
    ),
    (
        "reserve_body_available_with_owner",
        _RUNTIME_PRODUCTION_SERIALIZED_IMPL,
        """                retained_owner,
                retained_statement,""",
        """                ownership.owner().clone(),
                ownership.candidate_semantic_statement(),""",
        "an inexact unpublished retry must retain the exact incumbent lifecycle owner and effective authority",
    ),
)


def assert_folded_runtime_candidate_mutations(module, tmp_path: Path) -> None:
    """Run added candidate mutations under one existing collected selector."""

    for index, (item_name, context, old, new, diagnostic) in enumerate(
        _FOLDED_RUNTIME_CANDIDATE_MUTATIONS
    ):
        case_root = tmp_path / "folded-runtime-candidate" / str(index)
        repo_root, _formal_dir = copy_effect_capacity_mutation_fixture(
            case_root, module
        )
        runtime_path = repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
        source = runtime_path.read_text(encoding="utf-8")
        items = tuple(
            item
            for item in module.rust_items(source, item_name)
            if item.brace_context == context
        )
        assert len(items) == 1, (item_name, context)
        item = items[0]
        assert item.source.count(old) == 1, (item_name, old)
        start = source.index(item.source)
        runtime_path.write_text(
            source[:start]
            + item.source.replace(old, new, 1)
            + source[start + len(item.source) :],
            encoding="utf-8",
        )
        errors = module._effect_capacity_production_source_fidelity_errors(
            repo_root
        )
        assert any(diagnostic in error for error in errors), (
            diagnostic,
            errors,
        )


def assert_authenticated_wal_v4_negatives(
    module,
    tmp_path: Path,
) -> None:
    """Keep authenticated WAL replay checks independent of refreshed seals."""

    adapter_path = tmp_path / "crates/iroha_core/src/sumeragi/v2.rs"
    canonical_adapter = adapter_path.read_text(encoding="utf-8")
    adapter_name = "open_with_aggregator_and_publication_with_capacity"
    adapter_digest = module._SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256[
        adapter_name
    ]
    mutate_source_once(
        adapter_path,
        "registry.decode_wal_entry(\n"
        "                    record,\n",
        "registry.decode_wal_entry(\n"
        "                    record.payload(),\n",
    )
    mutated_adapter = adapter_path.read_text(encoding="utf-8")
    adapter_items = module.rust_function_items_from_structural(
        mutated_adapter,
        module.mask_rust_comments_and_literals(mutated_adapter),
        adapter_name,
    )
    assert len(adapter_items) == 1
    module._SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256[adapter_name] = (
        module._rust_item_token_sha256(adapter_items[0])
    )
    errors = module._serviced_candidate_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        "startup replay must decode the complete authenticated WAL record" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors
    module._SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256[adapter_name] = (
        adapter_digest
    )
    adapter_path.write_text(canonical_adapter, encoding="utf-8")

    regression_path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs"
    )
    canonical_regression = regression_path.read_text(encoding="utf-8")
    regression_name = (
        "post_wal_oversized_continuation_fails_closed_and_replays_exact_record"
    )
    regression_digest = (
        module._SERVICED_CANDIDATE_V4_ADAPTER_REGRESSION_TEST_SHA256[
            regression_name
        ]
    )
    mutate_source_once(
        regression_path,
        "adapter.wal.recovered_records()[0].sequence(), 0",
        "adapter.wal.recovered_records()[0].sequence().saturating_add(1), 0",
    )
    mutated_regression = regression_path.read_text(encoding="utf-8")
    regression_items = module.rust_function_items_from_structural(
        mutated_regression,
        module.mask_rust_comments_and_literals(mutated_regression),
        regression_name,
    )
    assert len(regression_items) == 1
    module._SERVICED_CANDIDATE_V4_ADAPTER_REGRESSION_TEST_SHA256[
        regression_name
    ] = module._rust_item_token_sha256(regression_items[0])
    errors = module._serviced_candidate_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        "the post-WAL oversized-continuation regression must inspect the authenticated record sequence"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors
    module._SERVICED_CANDIDATE_V4_ADAPTER_REGRESSION_TEST_SHA256[
        regression_name
    ] = regression_digest
    regression_path.write_text(canonical_regression, encoding="utf-8")


def rebind_remote_proposal_replay_mutation_digest(
    module,
    source_path: Path,
    item_name: str,
    expected_error: str,
) -> tuple[tuple[tuple[dict[str, str], str, str], ...], str] | None:
    """Return the rebound seal set and hash-only diagnostic for one replay seam."""

    generic = (
        ("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),
    )
    specs = {
        "authenticated Proposal dispatch must derive and transfer one exact replay origin": (
            (("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),),
            ((module._AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256, "runtime_driver_dispatch"),),
            "production authenticated runtime dispatch bridge declaration, contract",
        ),
        "authenticated dispatch matching must require the frozen physical ownership boundary": (
            (("impl", "RuntimeIngressOwnershipEvidence"),),
            ((module._AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256, "runtime_ingress_exactly_matches_authenticated"),),
            "post-authentication canonical payload comparator declaration",
        ),
        "retirement reconciliation must prune and validate deferred Proposal replay": (
            generic,
            ((module._AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256, "reconcile_deferred_runtime_ownership_after_retirement"),),
            "atomic deferred wrapper and orphan-receipt retirement reconciliation declaration",
        ),
        "driver acceptance must retain Proposal replay with its exact deferred ingress owner": (
            generic,
            ((module._AUTHENTICATED_DEFERRED_OWNERSHIP_RUST_ITEM_SHA256, "accept_driver_dispatch"),),
            "driver dispatch ownership acceptance declaration",
        ),
        "deferred Proposal replay must rebind the selected ProposalReceived ingress before effect ownership": (
            generic,
            (
                (module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256, "dispatch_one_adapter_deferred"),
                (module._SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256, "dispatch_one_adapter_deferred"),
            ),
            "single adapter-deferred runtime dispatcher declaration, contract",
        ),
    }
    spec = specs.get(expected_error)
    if spec is None:
        return None
    context, bindings, digest_diagnostic = spec
    return (
        rebind_reviewed_rust_item_digests(
            module, source_path, item_name, context, bindings
        ),
        digest_diagnostic,
    )


def test_same_view_generation_owner_purge_survives_runtime_digest_refresh(
    tmp_path: Path,
) -> None:
    """EnterView must evict dormant clock owners by the complete round tag."""

    module = load_checker()
    copy_serviced_candidate_production_fixture(tmp_path)
    runtime_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    mutate_rust_item_source(
        module,
        runtime_path,
        "observe_effects",
        """self.dormant_fresh_lifecycle_owners
                    .retain(|_, owner| owner.causal_origin().root_tag == tag);""",
        """self.dormant_fresh_lifecycle_owners.retain(|_, owner| {
                    let root = owner.causal_origin().root_tag;
                    root.height() == tag.height() && root.view() == tag.view()
                });""",
    )
    mutated = runtime_path.read_text(encoding="utf-8")
    items = module.rust_function_items_from_structural(
        mutated,
        module.mask_rust_comments_and_literals(mutated),
        "observe_effects",
    )
    assert len(items) == 1
    module._SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256["observe_effects"] = (
        module._rust_item_token_sha256(items[0])
    )

    errors = module._serviced_candidate_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        "EnterView must retire every stale full round-tag clock owner" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def exact_output_production_fixture(tmp_path: Path) -> None:
    """Copy every production source consumed by the exact-output checker."""

    for relative in (
        Path("crates/iroha_core/src/lib.rs"),
        Path("crates/iroha_core/src/merge_sidecar.rs"),
        Path("crates/iroha_core/src/sumeragi/mod.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/height_ingress_bindings.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_authority.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs"),
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
        Path("crates/iroha_test_network/src/lib.rs"),
        Path("crates/izanami/src/chaos.rs"),
        Path("crates/iroha_kagami/src/localnet.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)


def test_lifecycle_capacity_production_source_and_mutations_are_bound(
    tmp_path: Path,
) -> None:
    """Bind config admission, shipped profiles, and the runtime slot geometry."""

    module = load_checker()
    current_errors = module._lifecycle_capacity_production_source_fidelity_errors(
        ROOT_DIR
    )
    assert current_errors == [], current_errors

    mutations = (
        (
            "crates/iroha_config/src/parameters/actual.rs",
            ".checked_mul(certified_request_capacity)",
            ".saturating_mul(certified_request_capacity)",
            "checked consensus, observer, two-phase Serve, and Producer geometry",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "actual::validate_sumeragi_v2_lifecycle_capacity_geometry(",
            "actual::validate_sumeragi_v2_exact_output_geometry(",
            "root parsing must reject lifecycle geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lifecycle_authority.rs",
            "validate_sumeragi_v2_lifecycle_capacity_geometry(",
            "validate_sumeragi_v2_exact_output_geometry(",
            "runtime lifecycle authority must consume the shared checked geometry",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const CORE_MAX_TOTAL_CONNECTIONS: usize = 97;",
            "pub const CORE_MAX_TOTAL_CONNECTIONS: usize = 98;",
            "reviewed 97-source boundary",
        ),
        (
            "crates/iroha_test_network/src/lib.rs",
            "cap, None,",
            "cap, None,\n            .write([\"sumeragi\", \"queues\", \"bodies\"], 512i64)",
            "test networks must not restore the lifecycle-invalid 512-body override",
        ),
        (
            "crates/izanami/src/chaos.rs",
            "const IZANAMI_MAX_TOTAL_CONNECTIONS: i64 = 31;",
            "const IZANAMI_MAX_TOTAL_CONNECTIONS: i64 = 32;",
            "Izanami's 512-body profile must retain its reviewed 31-source cap",
        ),
        (
            "crates/iroha_kagami/src/localnet.rs",
            "const LOCALNET_SUMERAGI_QUEUE_COMMANDS: usize = 8_192;",
            "const LOCALNET_SUMERAGI_QUEUE_COMMANDS: usize = 8_191;",
            "Kagami must retain the reviewed high-command localnet profile",
        ),
    )
    for index, (relative, old, new, expected_error) in enumerate(mutations):
        fixture_root = tmp_path / f"mutation_{index}"
        exact_output_production_fixture(fixture_root)
        path = fixture_root / relative
        source = path.read_text(encoding="utf-8")
        assert source.count(old) == 1, (relative, old)
        path.write_text(source.replace(old, new, 1), encoding="utf-8")

        errors = module._lifecycle_capacity_production_source_fidelity_errors(
            fixture_root
        )
        assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "autonomous_lane_output_has_durable_reconstruction_source",
            "if proposal_height != artifact.height {",
            "if proposal_height == artifact.height {",
            "only a current-height noncanonical autonomous output may fall back",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            "|| durable_lane_authority.winning_proposal_hash(durable_proposal_hash)",
            "&& durable_lane_authority.winning_proposal_hash(durable_proposal_hash)",
            "same-finality nonwinning authority",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            """let bound_supersession_source = durable_lane_authority.covered_source_hash(
        artifact,
        &BlockMessage::LaneBlockProposal(durable_payload.origin_proposal.clone()),
    )?;""",
            "let bound_supersession_source = Some(durable_proposal_hash);",
            "same-finality nonwinning authority",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            ".read_autonomous_lane_retired_attempt(",
            ".read_autonomous_lane_block_artifact(",
            "immutable exact attempt",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            """if retired.retirement
        != crate::kura::AutonomousLaneSlotRetirementV1::from_payload(durable_payload)""",
            """if retired.retirement
        == crate::kura::AutonomousLaneSlotRetirementV1::from_payload(durable_payload)""",
            "exact retirement equality",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            "|| payload != durable_payload",
            "|| payload.payload_hash != durable_payload.payload_hash",
            "compare the exact local durable payload",
        ),
        (
            "autonomous_new_view_body_matches_durable_payload",
            ".is_ok_and(|expected| expected == *body)",
            ".is_ok_and(|_| true)",
            "compare the exact regenerated body",
        ),
        (
            "autonomous_lane_output_has_exact_retirement_source",
            ".find(|stored| stored.certificate == *certificate)",
            ".find(|_| true)",
            "find the exact durable certificate",
        ),
    ),
)
def test_autonomous_retirement_mutations_survive_item_digest_refresh(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Refreshed helper seals cannot hide weakened autonomous retirement."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    component = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_worker"
        / "autonomous_lane_output_retirement.rs"
    )
    mutate_rust_item_source(module, component, item_name, old, new)
    original = rebind_reviewed_rust_item_digests(
        module,
        component,
        item_name,
        (),
        ((module._PRODUCTION_EXACT_OUTPUT_ITEM_SHA256, item_name),),
    )
    try:
        errors = module._autonomous_retirement_source_contract_errors(tmp_path)
    finally:
        restore_reviewed_rust_item_digests(original)

    assert any(expected_error in error for error in errors), errors
    assert not any(
        f"exact-output {item_name} production item declaration and complete "
        "control flow" in error
        for error in errors
    ), errors


def test_autonomous_retirement_atomic_regression_survives_digest_refresh(
    tmp_path: Path,
) -> None:
    """The sealed regression must assert atomic pending ownership on failure."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    regression = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_worker"
        / "applied_height_handoff_tests.rs"
    )
    item_name = (
        "applied_height_handoff_retires_exact_noncanonical_autonomous_outputs_only"
    )
    mutate_rust_item_source(
        module,
        regression,
        item_name,
        'assert!(mutated.is_pending(), "failed handoff remains atomic");',
        'assert!(!mutated.is_pending(), "failed handoff remains atomic");',
    )
    original = rebind_reviewed_rust_item_digests(
        module,
        regression,
        item_name,
        (),
        ((module._AUTONOMOUS_RETIREMENT_HANDOFF_TEST_SHA256, item_name),),
    )
    try:
        errors = module._autonomous_retirement_source_contract_errors(tmp_path)
    finally:
        restore_reviewed_rust_item_digests(original)

    assert any(
        "preserve pending state after an inexact retirement failure" in error
        for error in errors
    ), errors
    assert not any(
        "autonomous-lane retirement release regression declaration and complete control flow"
        in error
        for error in errors
    ), errors


def test_restart_physical_high_water_mutation_survives_item_digest_refresh(
    tmp_path: Path,
) -> None:
    """Restart must publish the restored physical high-water before admission."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    ingress_path = tmp_path / "crates/iroha_core/src/sumeragi/mod.rs"
    source = ingress_path.read_text(encoding="utf-8")
    items = [
        item
        for item in module.rust_items(source, "bind_leader_wire_lifecycle_gate")
        if item.brace_context == (("impl", "FairV2Ingress"),)
    ]
    assert len(items) == 1
    item = items[0]
    old = ".max(restore.last_admission_ordinal());"
    new = ".max(0);"
    assert item.source.count(old) == 1
    mutated_item_source = item.source.replace(old, new, 1)
    ingress_path.write_text(
        source.replace(item.source, mutated_item_source, 1),
        encoding="utf-8",
    )

    mutated_item = next(
        candidate
        for candidate in module.rust_items(
            ingress_path.read_text(encoding="utf-8"),
            "bind_leader_wire_lifecycle_gate",
        )
        if candidate.brace_context == (("impl", "FairV2Ingress"),)
    )
    module._LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
        "bind_leader_wire_lifecycle_gate"
    ] = module._rust_item_token_sha256(mutated_item)

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )
    assert any(
        "restart binding must preserve the durable physical admission "
        "high-watermark before any fresh carrier allocation"
        in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            ".map(|(source, lane)| (source.clone(), lane.entries.len()))",
            ".map(|(source, _lane)| (source.clone(), 0))",
            "complete current physical source prefix",
        ),
        (
            "incumbent.ingress_predecessors = ingress_predecessors;",
            "incumbent.ingress_predecessors.clear();",
            "freshly frozen physical prefix",
        ),
    ),
)
def test_dormant_leader_wire_reactivation_mutations_survive_item_digest_refresh(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """A refreshed admission seal still freezes and installs the live prefix."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    ingress_path = tmp_path / "crates/iroha_core/src/sumeragi/mod.rs"
    mutate_rust_item_source(
        module,
        ingress_path,
        "fair_v2_ingress_admit_leader_wire",
        old,
        new,
    )
    mutated_items = module.rust_items(
        ingress_path.read_text(encoding="utf-8"),
        "fair_v2_ingress_admit_leader_wire",
    )
    assert len(mutated_items) == 1
    module._LEADER_WIRE_PHYSICAL_INGRESS_ITEM_SHA256[
        "fair_v2_ingress_admit_leader_wire"
    ] = module._rust_item_token_sha256(mutated_items[0])

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(expected_error in error for error in errors), errors

@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "assert!(earlier_ordinal < retry_ordinal);",
            "assert!(earlier_ordinal > retry_ordinal);",
            "ordinary physical predecessor before the restored lifecycle's fresh carrier",
        ),
        (
            "                })\n                .is_none(),\n",
            "                })\n                .is_some(),\n",
            "reject target-only selection while its frozen physical predecessor remains",
        ),
        (
            "payload: wire::ConsensusMessageV2Payload::CommitCertificateRequest(_),",
            "payload: wire::ConsensusMessageV2Payload::CommitCertificateResponse(_),",
            "drain and identify the frozen ordinary predecessor before the replay",
        ),
        (
            "ownership.leader_wire_token() == Some(&fixture.token)",
            "ownership.leader_wire_token().is_some()",
            "drain the exact restored owner only after its frozen physical prefix",
        ),
    ),
)
def test_restored_productive_retry_mutations_survive_regression_digest_refresh(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """The refreshed regression seal cannot hide loss of physical-prefix order."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    ingress_path = tmp_path / "crates/iroha_core/src/sumeragi/mod.rs"
    name = "restored_productive_retry_freezes_the_current_physical_source_prefix"
    mutate_rust_item_source(module, ingress_path, name, old, new)
    mutated_items = module.rust_items(
        ingress_path.read_text(encoding="utf-8"), name
    )
    assert len(mutated_items) == 1
    module._LEADER_WIRE_PHYSICAL_INGRESS_REGRESSION_TEST_SHA256[
        name
    ] = module._rust_item_token_sha256(mutated_items[0])

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(expected_error in error for error in errors), errors


def rebind_changed_same_round_expanded_source_seal(
    module, repo_root: Path
) -> None:
    """Rebind direct-fixture source seals before checking one semantic mutation."""

    rebound_relatives: list[str] = []
    for relative, expected_sha256 in (
        module._SAME_ROUND_SEMANTIC_KERNEL_SOURCE_SHA256.items()
    ):
        expansion_errors: list[str] = []
        _path, source = module._read_reviewed_rust_source(
            repo_root,
            relative,
            expansion_errors,
            "same-round semantic kernel mutation fixture",
        )
        assert not expansion_errors, expansion_errors
        observed_sha256 = hashlib.sha256(source.encode("utf-8")).hexdigest()
        if observed_sha256 != expected_sha256:
            module._SAME_ROUND_SEMANTIC_KERNEL_SOURCE_SHA256[relative] = (
                observed_sha256
            )
            rebound_relatives.append(relative)
    # Synthetic fixtures intentionally use the narrow direct-include reader,
    # while production seals use the authenticated recursive closure. Nested
    # split parents can therefore differ before the one deliberate mutation.
    assert rebound_relatives, rebound_relatives
    errors = module._same_round_semantic_kernel_source_fidelity_errors(repo_root)
    assert not any(
        "same-round semantic kernel source must match exact reviewed SHA-256"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_kind", "seal_key", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "struct",
            "V2IoCompletionOwnership",
            "V2IoCompletionOwnership",
            (),
            "    runtime_lifecycle_ordinal: Option<u128>,\n",
            "    runtime_lifecycle_owner: Option<u128>,\n",
            "completion ownership must retain time/debt, runtime-capacity class",
        ),
        (
            "struct",
            "V2IoCompletionOwnership",
            "V2IoCompletionOwnership",
            (),
            "    recovered_lifecycle_sign: Option<RecoveredLifecycleSignDispatchKeyV1>,\n",
            "    recovered_lifecycle_sign: Option<u128>,\n",
            "completion ownership must retain time/debt, runtime-capacity class",
        ),
        (
            "struct",
            "V2IoCompletionOwnership",
            "V2IoCompletionOwnership",
            (),
            "    recovered_decision_fetch: Option<RecoveredDecisionFetchDispatchKeyV1>,\n",
            "    recovered_decision_fetch: Option<u128>,\n",
            "completion ownership must retain time/debt, runtime-capacity class",
        ),
        (
            "struct",
            "V2IoCommandQueueState",
            "V2IoCommandQueueState",
            (),
            "    recovered_lifecycle_signs:\n"
            "        BTreeMap<RecoveredLifecycleSignDispatchKeyV1, V2IoTrackedRecoveredLifecycleSignV1>,\n",
            "    recovered_lifecycle_signs: BTreeMap<u128, V2IoTrackedRecoveredLifecycleSignV1>,\n",
            "command queue must retain every recovered lifecycle command under its exact opaque dispatch key",
        ),
        (
            "struct",
            "V2IoCommandQueueState",
            "V2IoCommandQueueState",
            (),
            "    recovered_decision_fetch_bodies:\n"
            "        BTreeMap<RecoveredDecisionFetchDispatchKeyV1, V2IoTrackedRecoveredDecisionFetchBodyV1>,\n",
            "    recovered_decision_fetch_bodies: BTreeMap<u128, V2IoTrackedRecoveredDecisionFetchBodyV1>,\n",
            "command queue must retain every recovered lifecycle command under its exact opaque dispatch key",
        ),
        (
            "struct",
            "V2IoCompletionOwnership",
            "V2IoCompletionOwnership",
            (),
            "    recovered_decision_apply: Option<RecoveredDecisionApplyDispatchKeyV1>,\n",
            "    recovered_decision_apply: Option<u128>,\n",
            "completion ownership must retain time/debt, runtime-capacity class",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::Sign { task, .. } => Some(task.lifecycle_ordinal()),\n",
            "            Self::Sign { .. } => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::RecoveredLifecycleSign(task) => Some(task.dispatch_key().lifecycle_ordinal()),\n",
            "            Self::RecoveredLifecycleSign(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::PersistRecoveredDecisionFetchBody(task) => {\n"
            "                Some(task.dispatch_key().lifecycle_ordinal())\n"
            "            }\n",
            "            Self::PersistRecoveredDecisionFetchBody(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::RecoveredDecisionApply(task) => Some(task.dispatch_key().lifecycle_ordinal()),\n",
            "            Self::RecoveredDecisionApply(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::Store(task) => Some(task.lifecycle_ordinal()),\n",
            "            Self::Store(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::Validate(task) => Some(task.lifecycle_ordinal()),\n",
            "            Self::Validate(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoCommand::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "V2IoCommand"),),
            "            Self::Apply(task) => Some(task.lifecycle_ordinal()),\n",
            "            Self::Apply(_) => None,\n",
            "every completion-producing I/O command must project its immutable runtime lifecycle ordinal",
        ),
        (
            "method",
            "V2IoAdmission::retain_completion",
            "retain_completion",
            (("impl", "V2IoAdmission"),),
            "            requires_runtime_capacity,\n"
            "            runtime_lifecycle_ordinal,\n",
            "            requires_runtime_capacity: false,\n"
            "            runtime_lifecycle_ordinal,\n",
            "completion publication must atomically retain the exact capacity class",
        ),
        (
            "method",
            "V2IoAdmission::retain_completion",
            "retain_completion",
            (("impl", "V2IoAdmission"),),
            "            recovered_lifecycle_sign,\n"
            "            recovered_decision_fetch,\n",
            "            recovered_lifecycle_sign: None,\n"
            "            recovered_decision_fetch,\n",
            "completion publication must atomically retain the exact capacity class",
        ),
        (
            "method",
            "V2IoAdmission::retain_completion",
            "retain_completion",
            (("impl", "V2IoAdmission"),),
            "            recovered_decision_fetch,\n"
            "        });\n",
            "            recovered_decision_fetch: None,\n"
            "        });\n",
            "completion publication must atomically retain the exact capacity class",
        ),
        (
            "method",
            "V2IoAdmission::retain_completion",
            "retain_completion",
            (("impl", "V2IoAdmission"),),
            "            runtime_lifecycle_ordinal,\n"
            "            recovered_decision_apply,\n",
            "            runtime_lifecycle_ordinal: None,\n"
            "            recovered_decision_apply,\n",
            "completion publication must atomically retain the exact capacity class",
        ),
        (
            "method",
            "V2IoAdmission::abandon_latest_completion",
            "abandon_latest_completion",
            (("impl", "V2IoAdmission"),),
            "            .pop_back()\n",
            "            .pop_front()\n",
            "failed send must abandon only the just-retained completion tail",
        ),
        (
            "method",
            "V2IoAdmission::completion_ownership_at",
            "completion_ownership_at",
            (("impl", "V2IoAdmission"),),
            "            .get(position)\n",
            "            .front()\n",
            "completion ownership projection must copy the exact indexed record without consuming it",
        ),
        (
            "method",
            "V2IoHandle::completion_ownership_at",
            "completion_ownership_at",
            (("impl", "V2IoHandle"),),
            "        self.admission.completion_ownership_at(position)\n",
            "        self.admission.completion_ownership_at(0)\n",
            "I/O handle must delegate the exact non-consuming ownership position",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                    let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal();\n",
            "                    let runtime_lifecycle_ordinal = None;\n",
            "I/O worker must capture exact completion provenance before moving",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                    let recovered_lifecycle_sign_key = command.recovered_lifecycle_sign_key();\n",
            "                    let recovered_lifecycle_sign_key = None;\n",
            "I/O worker must capture exact completion provenance before moving",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                    let recovered_decision_fetch_key = command.recovered_decision_fetch_key();\n",
            "                    let recovered_decision_fetch_key = None;\n",
            "I/O worker must capture exact completion provenance before moving",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                    let recovered_decision_apply_key = command.recovered_decision_apply_key();\n",
            "                    let recovered_decision_apply_key = None;\n",
            "I/O worker must capture exact completion provenance before moving",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                                            recovered_decision_apply_key.map_or_else(\n",
            "                                            None.map_or_else(\n",
            "I/O worker must use the key captured before execution",
        ),
        (
            "method",
            "V2IoHandle::spawn",
            "spawn",
            (("impl", "V2IoHandle"),),
            "                                            send_completion_with_lifecycle_ordinal(\n"
            "                                                &completion_tx,\n"
            "                                                &worker_admission,\n"
            "                                                Ok(completion),\n"
            "                                                runtime_lifecycle_ordinal,\n"
            "                                            );\n",
            "                                            send_completion_with_lifecycle_ordinal(\n"
            "                                                &completion_tx,\n"
            "                                                &worker_admission,\n"
            "                                                Ok(completion),\n"
            "                                                None,\n"
            "                                            );\n",
            "I/O worker must forward the pre-execution runtime lifecycle ordinal unchanged",
        ),
        (
            "method",
            "LocalCompletion::runtime_lifecycle_ordinal",
            "runtime_lifecycle_ordinal",
            (("impl", "LocalCompletion"),),
            "            Self::Reconstructed { task, .. } => task.lifecycle_ordinal(),\n",
            "            Self::Reconstructed { .. } => 0,\n",
            "every local completion must project the immutable lifecycle ordinal",
        ),
        (
            "method",
            "send_completion_with_lifecycle_ordinal",
            "send_completion_with_lifecycle_ordinal",
            (),
            "        runtime_lifecycle_ordinal,\n"
            "    );\n",
            "        None,\n"
            "    );\n",
            "production completion wrapper must forward the captured runtime lifecycle ordinal unchanged",
        ),
        (
            "method",
            "send_tracked_completion_with_lifecycle_ordinal",
            "send_tracked_completion_with_lifecycle_ordinal",
            (),
            "        runtime_lifecycle_ordinal,\n"
            "        recovered_decision_apply,\n",
            "        None,\n"
            "        recovered_decision_apply,\n",
            "blocking completion publication must retain exact ownership before send",
        ),
        (
            "method",
            "send_tracked_completion_with_lifecycle_ordinal",
            "send_tracked_completion_with_lifecycle_ordinal",
            (),
            "        admission.abandon_latest_completion();\n",
            "        let _ = admission;\n",
            "blocking completion publication must retain exact ownership before send",
        ),
        (
            "method",
            "try_send_tracked_completion_with_lifecycle_ordinal",
            "try_send_tracked_completion_with_lifecycle_ordinal",
            (),
            "        runtime_lifecycle_ordinal,\n"
            "        recovered_decision_apply,\n",
            "        None,\n"
            "        recovered_decision_apply,\n",
            "nonblocking completion publication must retain exact ownership before send",
        ),
        (
            "method",
            "try_send_tracked_completion_with_lifecycle_ordinal",
            "try_send_tracked_completion_with_lifecycle_ordinal",
            (),
            "        admission.abandon_latest_completion();\n",
            "        let _ = admission;\n",
            "nonblocking completion publication must retain exact ownership before send",
        ),
    ),
)
def test_exact_serve_completion_provenance_survives_digest_refresh(
    tmp_path: Path,
    item_kind: str,
    seal_key: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Completion provenance remains exact after each individual item reseal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        items = module.rust_struct_items(source, item_name)
    else:
        items = tuple(
            item
            for item in module.rust_items(source, item_name)
            if item.brace_context == context
        )
    assert len(items) == 1, (seal_key, [item.brace_context for item in items])
    item = items[0]
    assert item.source.count(old) == 1, (seal_key, old)
    path.write_text(
        source.replace(item.source, item.source.replace(old, new, 1), 1),
        encoding="utf-8",
    )
    mutated_source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        mutated_items = module.rust_struct_items(mutated_source, item_name)
        seal_group = module._DIRECT_SERVE_WORKER_STRUCT_SHA256
    else:
        mutated_items = tuple(
            candidate
            for candidate in module.rust_items(mutated_source, item_name)
            if candidate.brace_context == context
        )
        seal_group = module._DIRECT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256
    assert len(mutated_items) == 1
    seal_group[seal_key] = module._rust_item_token_sha256(mutated_items[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = module._direct_serve_predecessor_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    ("seal_key", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "build_v2_io_command_channel",
            "build_v2_io_command_channel",
            (),
            "            producer_episode_due: false,\n",
            "            producer_episode_due: true,\n",
            "the command channel initializer must clear producer-episode due "
            "immediately before active",
        ),
        (
            "build_v2_io_command_channel",
            "build_v2_io_command_channel",
            (),
            "            recovered_decision_applies: BTreeMap::new(),\n",
            "            recovered_decision_applies: BTreeMap::default(),\n",
            "command channel initializer must start with no fabricated recovered Decision Apply owner",
        ),
        (
            "build_v2_io_command_channel",
            "build_v2_io_command_channel",
            (),
            "            recovered_lifecycle_signs: BTreeMap::new(),\n",
            "            recovered_lifecycle_signs: BTreeMap::default(),\n",
            "command channel initializer must start with no fabricated recovered Sign owner",
        ),
        (
            "build_v2_io_command_channel",
            "build_v2_io_command_channel",
            (),
            "            recovered_decision_fetch_bodies: BTreeMap::new(),\n",
            "            recovered_decision_fetch_bodies: BTreeMap::default(),\n",
            "command channel initializer must start with no fabricated recovered Decision Fetch owner",
        ),
        (
            "build_v2_io_command_channel",
            "build_v2_io_command_channel",
            (),
            "            producer_episode_due: false,\n"
            "            producer_episode_active: false,\n",
            "            producer_episode_active: false,\n"
            "            producer_episode_due: false,\n",
            "the command channel initializer must clear producer-episode due "
            "immediately before active",
        ),
        (
            "V2IoCommandQueue::close_receiver",
            "close_receiver",
            (("impl", "V2IoCommandQueue"),),
            "        state.producer_episode_due = false;\n",
            "        state.producer_episode_due = true;\n",
            "receiver teardown must clear producer-episode due before active "
            "and Serve rollback",
        ),
        (
            "V2IoCommandQueue::close_receiver",
            "close_receiver",
            (("impl", "V2IoCommandQueue"),),
            "        state\n"
            "            .recovered_decision_applies\n"
            "            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);\n",
            "        state.recovered_decision_applies.clear();\n",
            "receiver teardown must retain only completion-pending recovered Decision Apply ownership",
        ),
        (
            "V2IoCommandQueue::close_receiver",
            "close_receiver",
            (("impl", "V2IoCommandQueue"),),
            "        state\n"
            "            .recovered_lifecycle_signs\n"
            "            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);\n",
            "        state.recovered_lifecycle_signs.clear();\n",
            "receiver teardown must retain only completion-pending recovered Sign ownership",
        ),
        (
            "V2IoCommandQueue::close_receiver",
            "close_receiver",
            (("impl", "V2IoCommandQueue"),),
            "        state\n"
            "            .recovered_decision_fetch_bodies\n"
            "            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);\n",
            "        state.recovered_decision_fetch_bodies.clear();\n",
            "receiver teardown must retain only completion-pending recovered Decision Fetch ownership",
        ),
        (
            "V2IoCommandQueue::close_receiver",
            "close_receiver",
            (("impl", "V2IoCommandQueue"),),
            "        state.producer_episode_due = false;\n"
            "        state.producer_episode_active = false;\n",
            "        state.producer_episode_active = false;\n"
            "        state.producer_episode_due = false;\n",
            "receiver teardown must clear producer-episode due before active "
            "and Serve rollback",
        ),
    ),
)
def test_exact_serve_channel_lifecycle_boundaries_survive_item_digest_refresh(
    tmp_path: Path,
    seal_key: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Channel initialization and teardown remain semantic after resealing."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    items = tuple(
        item
        for item in module.rust_items(source, item_name)
        if item.brace_context == context
    )
    assert len(items) == 1
    item = items[0]
    assert item.source.count(old) == 1, (seal_key, old)
    path.write_text(
        source.replace(item.source, item.source.replace(old, new, 1), 1),
        encoding="utf-8",
    )
    mutated_items = tuple(
        candidate
        for candidate in module.rust_items(
            path.read_text(encoding="utf-8"), item_name
        )
        if candidate.brace_context == context
    )
    assert len(mutated_items) == 1
    module._DIRECT_SERVE_WORKER_ITEM_SHA256[seal_key] = (
        module._rust_item_token_sha256(mutated_items[0])
    )

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors
def test_exact_serve_producer_episode_must_use_survives_digest_refresh(
    tmp_path: Path,
) -> None:
    """The producer lease cannot lose its must-use boundary behind resealing."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    old = "#[must_use]\npub(crate) struct CertifiedServeProducerEpisode {\n"
    new = "pub(crate) struct CertifiedServeProducerEpisode {\n"
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")
    items = module.rust_struct_items(
        path.read_text(encoding="utf-8"),
        "CertifiedServeProducerEpisode",
    )
    assert len(items) == 1
    module._DIRECT_SERVE_WORKER_STRUCT_SHA256[
        "CertifiedServeProducerEpisode"
    ] = module._rust_item_token_sha256(items[0])

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        "exact-Serve state carrier CertifiedServeProducerEpisode must have "
        "exact reviewed attributes"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors

def test_exact_serve_producer_episode_drop_survives_digest_refresh(
    tmp_path: Path,
) -> None:
    """A refreshed producer lease still clears active under the queue lock."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    context = (("impl", "Drop", "for", "CertifiedServeProducerEpisode"),)
    items = tuple(
        item
        for item in module.rust_items(source, "drop")
        if item.brace_context == context
    )
    assert len(items) == 1
    item = items[0]
    old = "        if !state.producer_episode_active {\n"
    new = "        if state.producer_episode_active {\n"
    assert item.source.count(old) == 1
    path.write_text(
        source.replace(item.source, item.source.replace(old, new, 1), 1),
        encoding="utf-8",
    )
    mutated_items = tuple(
        candidate
        for candidate in module.rust_items(
            path.read_text(encoding="utf-8"), "drop"
        )
        if candidate.brace_context == context
    )
    assert len(mutated_items) == 1
    module._DIRECT_SERVE_WORKER_ITEM_SHA256[
        "CertifiedServeProducerEpisode::drop"
    ] = module._rust_item_token_sha256(mutated_items[0])

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        "ordinary producer episodes must retire under the same queue lock"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def test_exact_serve_boolean_projection_remains_test_only_after_digest_refresh(
    tmp_path: Path,
) -> None:
    """The legacy boolean projection cannot re-enter the production runtime."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    source = path.read_text(encoding="utf-8")
    old = (
        "    #[cfg(test)]\n"
        "    pub(crate) fn older_lifecycle_predates_exact_serve(\n"
    )
    new = "    pub(crate) fn older_lifecycle_predates_exact_serve(\n"
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    context = (
        ("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),
    )
    items = tuple(
        item
        for item in module.rust_items(
            path.read_text(encoding="utf-8"),
            "older_lifecycle_predates_exact_serve",
        )
        if item.brace_context == context
    )
    assert len(items) == 1
    module._DIRECT_SERVE_RUNTIME_ITEM_SHA256[
        "older_lifecycle_predates_exact_serve"
    ] = module._rust_item_token_sha256(items[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )
    assert any(
        "exact-Serve runtime seam older_lifecycle_predates_exact_serve "
        "must have exact reviewed attributes"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def test_exact_serve_executor_duplicate_boolean_seam_stays_absent(
    tmp_path: Path,
) -> None:
    """Production cannot regain a second mutable projection of witness state."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    source = path.read_text(encoding="utf-8")
    marker = (
        "    /// Publish executor-retained owners and compare the retained-response\n"
    )
    assert source.count(marker) == 1
    duplicate = """    pub(crate) fn older_runtime_lifecycle_predates_exact_serve(
        &mut self,
        now: Instant,
        serve_lifecycle_ordinal: u128,
    ) -> Result<bool, EffectExecutorError> {
        self.ensure_open()?;
        self.publish_external_lifecycle_owners()?;
        self.runtime
            .older_lifecycle_predates_exact_serve(now, serve_lifecycle_ordinal)
            .map_err(EffectExecutorError::Runtime)
    }

"""
    path.write_text(source.replace(marker, duplicate + marker, 1), encoding="utf-8")
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )
    assert any(
        "duplicate executor boolean projection must remain absent" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("seal_key", "item_name", "old", "new", "expected_error"),
    (
        (
            "V2IoCommandQueue::retire_selected_serve_ingress_occurrence",
            "retire_selected_serve_ingress_occurrence",
            "            state.producer_episode_due = true;\n",
            "            state.producer_episode_due = false;\n",
            "final frozen Serve retirement must atomically arm exactly one producer episode",
        ),
        (
            "V2IoCommandQueue::reserve_serve_ingress",
            "reserve_serve_ingress",
            "        if state.producer_episode_due || state.producer_episode_active {\n",
            "        if state.producer_episode_active {\n",
            "fresh Serve admission must not cross a due or active producer episode",
        ),
        (
            "V2IoCommandQueue::try_begin_producer_episode",
            "try_begin_producer_episode",
            "        state.producer_episode_due = false;\n"
            "        state.producer_episode_active = true;\n",
            "        state.producer_episode_active = true;\n",
            "ordinary producers must consume the one-shot handoff",
        ),
    ),
)
def test_post_serve_producer_handoff_mutations_survive_item_digest_refresh(
    tmp_path: Path,
    seal_key: str,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """The due-to-active handoff remains semantic after refreshing its seal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    items = [
        item
        for item in module.rust_items(source, item_name)
        if item.brace_context == (("impl", "V2IoCommandQueue"),)
    ]
    assert len(items) == 1
    item = items[0]
    assert item.source.count(old) == 1, (seal_key, old)
    mutated_source = item.source.replace(old, new, 1)
    path.write_text(
        source.replace(item.source, mutated_source, 1),
        encoding="utf-8",
    )
    mutated_items = [
        candidate
        for candidate in module.rust_items(
            path.read_text(encoding="utf-8"), item_name
        )
        if candidate.brace_context == (("impl", "V2IoCommandQueue"),)
    ]
    assert len(mutated_items) == 1
    module._DIRECT_SERVE_WORKER_ITEM_SHA256[seal_key] = (
        module._rust_item_token_sha256(mutated_items[0])
    )

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors

def test_post_serve_producer_handoff_regression_survives_digest_refresh(
    tmp_path: Path,
) -> None:
    """The regression must exercise both the due and active Busy boundaries."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_worker_selected_serve_cases_02_tests.rs"
    )
    source = path.read_text(encoding="utf-8")
    name = (
        "final_serve_retirement_yields_one_producer_episode_before_replenishment"
    )
    context: tuple[tuple[str, ...], ...] = ()
    items = [
        item
        for item in module.rust_items(source, name)
        if item.brace_context == context
    ]
    assert len(items) == 1
    item = items[0]
    old = "        Err(CertifiedServeIngressReserveError::Busy)\n"
    new = "        Err(CertifiedServeIngressReserveError::Rejected)\n"
    assert item.source.count(old) == 2
    mutated_source = item.source.replace(old, new, 1)
    path.write_text(
        source.replace(item.source, mutated_source, 1),
        encoding="utf-8",
    )
    mutated_item = next(
        candidate
        for candidate in module.rust_items(
            path.read_text(encoding="utf-8"), name
        )
        if candidate.brace_context == context
    )
    module._DIRECT_SERVE_REGRESSION_TEST_SHA256[name] = (
        module._rust_item_token_sha256(mutated_item)
    )

    errors = (
        module._direct_serve_predecessor_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        "reject replenishment both before and during the producer episode"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "    if !recovering_interrupted_tip {\n",
            "    if !recovering_interrupted_tip && _predecessor_admission_open {\n",
        ),
        (
            "    if !recovering_interrupted_tip {\n"
            "        service(CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode)?;\n"
            "    }\n",
            "    service(CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode)?;\n",
        ),
        (
            "    service(CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix)?;\n",
            "    let _ = CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix;\n",
        ),
        (
            "        service(CertifiedServeBarrierLivenessAction::Pacemaker)\n",
            "        service(CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix)\n",
        ),
    ),
)
def test_selected_serve_liveness_helper_survives_own_digest_refresh(
    tmp_path: Path,
    old: str,
    new: str,
) -> None:
    """The sealed helper retains every action after its own digest refresh."""

    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    runner_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    source = runner_path.read_text(encoding="utf-8")
    items = module.rust_items(
        source,
        "service_certified_serve_barrier_liveness_turn",
    )
    assert len(items) == 1
    item = items[0]
    assert item.source.count(old) == 1, old
    mutated_item_source = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1
    runner_path.write_text(
        source.replace(item.source, mutated_item_source, 1),
        encoding="utf-8",
    )
    mutated_items = module.rust_items(
        runner_path.read_text(encoding="utf-8"),
        "service_certified_serve_barrier_liveness_turn",
    )
    assert len(mutated_items) == 1
    module._PRODUCTION_LOCAL_RUNNER_SERVICE_ITEM_SHA256[
        "service_certified_serve_barrier_liveness_turn"
    ] = module._rust_item_token_sha256(mutated_items[0])

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(
        "selected-Serve liveness service must admit TimeoutVote, drain its "
        "retained prefix, and run the pacemaker in reviewed order"
        in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors

def _assert_selected_serve_liveness_item_semantic_mutation(
    tmp_path: Path,
    seal_key: str,
    source_kind: str,
    item_kind: str,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Each sealed selected-Serve item retains a semantic negative mutation."""

    module = load_checker()
    assert len(
        module._PRODUCTION_SELECTED_SERVE_LIVENESS_REGRESSION_ITEM_SHA256
    ) == 17
    assert (
        seal_key
        in module._PRODUCTION_SELECTED_SERVE_LIVENESS_REGRESSION_ITEM_SHA256
    )
    formal_dir = local_runner_service_fixture(tmp_path, module)
    source_path = (
        tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
        if source_kind == "runner"
        else (
            tmp_path
            / "crates/iroha_core/src/sumeragi/tests/v2_runner_unsealed_00.rs"
            if source_kind == "runner_test"
            else (
                tmp_path
                / "crates/iroha_core/src/sumeragi/tests/"
                "v2_worker_equivocation_and_selected_serve_fixture.rs"
            )
        )
    )
    worker_test_context = ()
    worker_method_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "feature",
            "=",
            ")",
            "]",
            "impl",
            "SelectedServeTimeoutRecoveryFixture",
        ),
    )
    worker_drop_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "feature",
            "=",
            ")",
            "]",
            "impl",
            "Drop",
            "for",
            "SelectedServeTimeoutRecoveryFixture",
        ),
    )

    def selected_items(source: str):
        if item_kind == "enum":
            return module.rust_enum_items(source, item_name)
        if item_kind == "struct":
            return tuple(
                item
                for item in module.rust_struct_items(source, item_name)
                if item.brace_context == worker_test_context
            )
        context = (
            worker_drop_context if item_kind == "drop" else worker_method_context
        )
        candidates = module.rust_items(source, item_name)
        return (
            candidates
            if source_kind in {"runner", "runner_test"}
            else tuple(item for item in candidates if item.brace_context == context)
        )

    source = source_path.read_text(encoding="utf-8")
    items = selected_items(source)
    assert len(items) == 1
    item = items[0]
    if source_kind == "worker" and item.source.count(old) == 0:
        # The reviewed worker fixture moved from a nested inline module into
        # one include provider, removing exactly one four-space indent level.
        def extracted_provider_text(value: str) -> str:
            return "".join(
                line[4:] if line.startswith("    ") else line
                for line in value.splitlines(keepends=True)
            )

        old, new = extracted_provider_text(old), extracted_provider_text(new)
    assert item.source.count(old) == 1, (seal_key, old)
    mutated_item = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1, seal_key
    source_path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    mutated_items = selected_items(source_path.read_text(encoding="utf-8"))
    assert len(mutated_items) == 1
    module._PRODUCTION_SELECTED_SERVE_LIVENESS_REGRESSION_ITEM_SHA256[
        seal_key
    ] = module._rust_item_token_sha256(mutated_items[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    ("seal_key", "source_kind", "item_kind", "item_name", "old", "new", "expected_error"),
    (('runner::CertifiedServeBarrierLivenessAction',
      'runner',
      'enum',
      'CertifiedServeBarrierLivenessAction',
      '    Pacemaker,\n',
      '    PacemakerBypass,\n',
      'selected-Serve liveness action vocabulary must remain closed'),
     ('runner::closed_certified_serve_predecessor_admission_cannot_veto_pacemaker',
      'runner_test',
      'item',
      'closed_certified_serve_predecessor_admission_cannot_veto_pacemaker',
      '    service_certified_serve_barrier_pacemaker_turn(false, || {\n'
      '        calls.set(calls.get().saturating_add(1));\n'
      '        Ok::<(), ()>(())\n'
      '    })\n'
      '    .expect("live certified Serve barrier services one pacemaker turn");\n'
      '    assert_eq!(\n'
      '        calls.get(),\n'
      '        1,\n'
      '        "a closed predecessor admission cannot veto the live pacemaker"\n'
      '    );\n',
      '    service_certified_serve_barrier_pacemaker_turn(true, || {\n'
      '        calls.set(calls.get().saturating_add(1));\n'
      '        Ok::<(), ()>(())\n'
      '    })\n'
      '    .expect("live certified Serve barrier services one pacemaker turn");\n'
      '    assert_eq!(\n'
      '        calls.get(),\n'
      '        1,\n'
      '        "a closed predecessor admission cannot veto the live pacemaker"\n'
      '    );\n',
      'a closed selected-Serve predecessor admission must retain the bounded pacemaker turn'),
     ('worker::SelectedServeTimeoutRecoveryMode',
      'worker',
      'enum',
      'SelectedServeTimeoutRecoveryMode',
      '        LatePassiveFetch,\n',
      '        LatePassiveFetchBypass,\n',
      'selected-Serve fixture mode vocabulary must remain closed'),
     ('worker::SelectedServeLatePassiveFetch',
      'worker',
      'struct',
      'SelectedServeLatePassiveFetch',
      '        body_store: V2BodyStore,\n',
      '',
      'late-passive-Fetch fixture must retain the exact body store, immutable task owner, manifest, '
      'and body'),
     ('worker::SelectedServeTimeoutRecoveryFixture',
      'worker',
      'struct',
      'SelectedServeTimeoutRecoveryFixture',
      '        late_passive_fetch: Option<SelectedServeLatePassiveFetch>,\n',
      '',
      'selected-Serve regression must retain every real ingress, runtime, worker, and observation '
      'owner'),
     ('worker::SelectedServeTimeoutRecoveryFixture::new',
      'worker',
      'method',
      'new',
      '            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)\n',
      '            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)\n',
      'the timeout-recovery fixture constructor must select only its exact closed mode'),
     ('worker::SelectedServeTimeoutRecoveryFixture::new_late_passive_fetch',
      'worker',
      'method',
      'new_late_passive_fetch',
      '            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)\n',
      '            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)\n',
      'the late-passive-Fetch fixture constructor must select only its exact closed mode'),
     ('worker::SelectedServeTimeoutRecoveryFixture::new_for_mode',
      'worker',
      'method',
      'new_for_mode',
      '                    .take(2)\n',
      '                    .take(1)\n',
      'selected-Serve fixture must enqueue exactly two distinct remote timeout signers'),
     ('worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix',
      'worker',
      'method',
      'service_exact_serve_runtime_prefix',
      '            let _ = self\n'
      '                .services\n'
      '                .drain_exact_serve_runtime_predecessor(\n'
      '                    &mut self.executor,\n'
      '                    barrier.scheduler_ordinal(),\n'
      '                )\n'
      '                .map_err(|error| error.to_string())?;\n'
      '            let completion_evidence = self\n'
      '                .services\n'
      '                .certified_serve_predecessor_completion_evidence(\n'
      '                    self.executor.remaining_completion_capacity() != 0,\n'
      '                    barrier.scheduler_ordinal(),\n'
      '                )?;\n',
      '            let _ = self\n'
      '                .services\n'
      '                .drain_exact_serve_runtime_predecessor(\n'
      '                    &mut self.executor,\n'
      '                    barrier.scheduler_ordinal(),\n'
      '                )\n'
      '                .map_err(|error| error.to_string())?;\n'
      '            let completion_evidence = None;\n',
      'selected-Serve exact runtime prefix must open one direct-observation admission, drain the '
      'strict completion, service at most one capacity-gated predecessor, then re-observe and retire '
      'the move-only guard'),
     ('worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve',
      'worker',
      'method',
      'assert_late_passive_fetch_completion_reopens_selected_serve',
      '            assert!(matches!(\n'
      '                &validated,\n'
      '                BodyValidationCompletion::Rejected { work_id, reason }\n'
      '                    if *work_id == validation_task.id()\n'
      '                        && reason == "deterministic late-passive-Fetch rejection"\n'
      '            ));\n',
      '',
      'Validate must terminate deterministically through an exact tracked rejection completion rather '
      'than opening an unbounded Sign suffix'),
     ('worker::SelectedServeTimeoutRecoveryFixture::service_timeout_vote_episode',
      'worker',
      'method',
      'service_timeout_vote_episode',
      '                    FairV2IngressBarrierBypass::TimeoutVoteEpisode,\n',
      '                    FairV2IngressBarrierBypass::None,\n',
      'selected-Serve fixture must use only the reviewed direct TimeoutVote bypass predicate'),
     ('worker::SelectedServeTimeoutRecoveryFixture::service_timeout_recovery_prefix',
      'worker',
      'method',
      'service_timeout_recovery_prefix',
      '                        Some(lifecycle_ordinal),\n',
      '                        None,\n',
      'selected-Serve fixture local timeout signature must retain its tracked lifecycle ordinal'),
     ('worker::SelectedServeTimeoutRecoveryFixture::service_pacemaker',
      'worker',
      'method',
      'service_pacemaker',
      '.step_pacemaker_once(Instant::now(), &mut self.services)',
      '.step(Instant::now(), &mut self.services)',
      'selected-Serve fixture must run exactly one typed pacemaker transition at the live ingress cut'),
     ('worker::SelectedServeTimeoutRecoveryFixture::entered_view_one',
      'worker',
      'method',
      'entered_view_one',
      'self.executor.current_tag().view() == 1 && self.services.active_tag.view() == 1',
      'self.executor.current_tag().view() == 1',
      'selected-Serve fixture EnterView terminal must agree between reducer and production service'),
     ('worker::SelectedServeTimeoutRecoveryFixture::assert_complete',
      'worker',
      'method',
      'assert_complete',
      '            assert_eq!(self.remote_timeout_votes_admitted, 2);\n',
      '            assert_eq!(self.remote_timeout_votes_admitted, 1);\n',
      'selected-Serve fixture must retain the Serve and reach exact local plus dual-remote recovery '
      'counts'),
     ('worker::SelectedServeTimeoutRecoveryFixture::assert_missing_proposal_serve_selected',
      'worker',
      'method',
      'assert_missing_proposal_serve_selected',
      '            assert_eq!(barrier.request_hash(), self.missing_proposal_request_hash);\n',
      '            assert_ne!(barrier.request_hash(), self.missing_proposal_request_hash);\n',
      'selected-Serve fixture must retain the exact missing-proposal request owner'),
     ('worker::SelectedServeTimeoutRecoveryFixture::drop',
      'worker',
      'drop',
      'drop',
      '            drop(self.services.io.take());\n',
      '            let _ = self.services.io.as_ref();\n',
      'selected-Serve synchronous fixture teardown must detach its worker endpoints without a '
      'synthetic shutdown')),
)
def test_selected_serve_liveness_items_survive_individual_digest_refresh(
    tmp_path: Path,
    seal_key: str,
    source_kind: str,
    item_kind: str,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Each current sealed selected-Serve item retains one semantic mutation."""

    _assert_selected_serve_liveness_item_semantic_mutation(
        tmp_path, seal_key, source_kind, item_kind, item_name, old, new, expected_error
    )

def test_selected_serve_timeout_owner_freeze_must_precede_serve_ingress_after_digest_refresh(
    tmp_path: Path,
) -> None:
    """A refreshed fixture seal cannot move height-start timeout ownership after Serve."""

    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    worker_path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/"
        "v2_worker_equivocation_and_selected_serve_fixture.rs"
    )
    source = worker_path.read_text(encoding="utf-8")
    worker_method_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "feature",
            "=",
            ")",
            "]",
            "impl",
            "SelectedServeTimeoutRecoveryFixture",
        ),
    )
    methods = tuple(
        item
        for item in module.rust_items(source, "new_for_mode")
        if item.brace_context == worker_method_context
    )
    assert len(methods) == 1
    method = methods[0]
    freeze = (
        "                    let timeout_owner = executor\n"
        "                        .freeze_due_timeout_owner_for_test(Instant::now())\n"
        "                        .expect(\"freeze the height-start timeout before later Serve ingress\");\n"
        "                    assert_eq!(\n"
        "                        timeout_owner.lifecycle_ordinal(),\n"
        "                        1,\n"
        "                        \"the height-start timeout owns the first actor-global scheduler position\"\n"
        "                    );\n"
    )
    serve_ingress = (
        "            assert!(matches!(\n"
        "                ingress.try_push(certified_serve_inbound(\n"
        "                    missing_request.request(),\n"
        "                    authenticated_via,\n"
        "                )),\n"
        "                Ok(FairV2IngressPushDisposition::Enqueued)\n"
        "            ));\n"
    )
    assert method.source.count(freeze) == 1
    assert method.source.count(serve_ingress) == 1
    late_freeze = (
        "            if mode == SelectedServeTimeoutRecoveryMode::TimeoutRecovery {\n"
        + "".join(
            line[4:] if line.startswith("    ") else line
            for line in freeze.splitlines(keepends=True)
        )
        + "            }\n"
    )
    mutated_method = method.source.replace(freeze, "", 1)
    mutated_method = mutated_method.replace(
        serve_ingress,
        serve_ingress + "\n" + late_freeze,
        1,
    )
    assert source.count(method.source) == 1
    worker_path.write_text(
        source.replace(method.source, mutated_method, 1),
        encoding="utf-8",
    )
    mutated_source = worker_path.read_text(encoding="utf-8")
    mutated_methods = tuple(
        item
        for item in module.rust_items(mutated_source, "new_for_mode")
        if item.brace_context == worker_method_context
    )
    assert len(mutated_methods) == 1
    module._PRODUCTION_SELECTED_SERVE_LIVENESS_REGRESSION_ITEM_SHA256[
        "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode"
    ] = module._rust_item_token_sha256(mutated_methods[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(
        "height-start timeout owner must freeze before Serve ingress" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def _assert_direct_serve_item_mutation(
    tmp_path: Path,
    relative: str,
    item_kind: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Mutate one direct-Serve item and require the structural checker to reject it."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        items = module.rust_struct_items(source, item_name)
    else:
        items = tuple(
            item
            for item in module.rust_items(source, item_name)
            if item.brace_context == context
        )
    assert len(items) == 1, (relative, item_name, [item.brace_context for item in items])
    item = items[0]
    assert item.source.count(old) == 1, (relative, item_name, old)
    path.write_text(
        source.replace(item.source, item.source.replace(old, new, 1), 1),
        encoding="utf-8",
    )
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = module._direct_serve_predecessor_production_source_fidelity_errors(
        tmp_path
    )
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_kind", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "method",
            "matches_barrier",
            (("impl", "V2IoCertifiedServeIngressReservation"),),
            "        self.id.0 == barrier.scheduler_ordinal\n"
            "            && self.lifecycle_id == barrier.lifecycle_id\n",
            "        self.id.0 == barrier.scheduler_ordinal\n            && true\n",
            "barrier comparison must retain every logical and physical identity component",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "method",
            "open_serve_predecessor_admission",
            (("impl", "V2IoCommandQueue"),),
            "                predecessor_ordinal: None,\n",
            "                predecessor_ordinal: Some(barrier.scheduler_ordinal()),\n",
            "queue-local predecessor-admission open",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "method",
            "exact_serve_predecessor_observation",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        let predecessor = minimum.filter(|ordinal| *ordinal < serve_lifecycle_ordinal);\n",
            "        let predecessor = minimum.filter(|ordinal| *ordinal <= serve_lifecycle_ordinal);\n",
            "direct predecessor census",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "method",
            "service_certified_serve_barrier",
            (),
            "    let predecessor_admission = predecessor\n"
            "        .should_open_predecessor_admission()\n",
            "    let predecessor_admission = predecessor\n"
            "        .has_runnable_predecessor()\n",
            "ordinary direct selected-Serve predecessor turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "method",
            "service_pending_certified_serve_barrier",
            (),
            "            output_guard.close_admission_for_restart();\n"
            "            return Err(V2RunnerError::Service(\n"
            "                \"completed pending Kura recovery retained a runnable Serve predecessor\".to_owned(),\n",
            "            let _ = output_guard;\n"
            "            return Err(V2RunnerError::Service(\n"
            "                \"completed pending Kura recovery retained a runnable Serve predecessor\".to_owned(),\n",
            "pending-Kura direct selected-Serve predecessor turn",
        ),
    ),
)
def test_exact_serve_checker_boundaries_survive_item_digest_refresh(
    tmp_path: Path,
    relative: str,
    item_kind: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Direct observation and transient admission remain semantic after resealing."""

    _assert_direct_serve_item_mutation(
        tmp_path, relative, item_kind, item_name, context, old, new, expected_error
    )


@pytest.mark.parametrize(
    ("item_kind", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "struct",
            "ExactServePredecessorCompletionEvidence",
            (),
            "    lifecycle_ordinal_complement: u128,\n",
            "    lifecycle_ordinal_checksum: u128,\n",
            "completion evidence must retain a complemented immutable ordinal",
        ),
        (
            "method",
            "validate_exact",
            (("impl", "ExactServePredecessorCompletionEvidence"),),
            "        self.lifecycle_ordinal > 0 && self.lifecycle_ordinal_complement == !self.lifecycle_ordinal\n",
            "        self.lifecycle_ordinal > 0 && true\n",
            "completion evidence must reject zero and complement drift",
        ),
        (
            "struct",
            "ExactServePredecessorObservation",
            (),
            "    runnable_predecessor: bool,\n",
            "    runnable_successor: bool,\n",
            "direct observation must retain exactly its initial-turn and runnable-prefix facts",
        ),
        (
            "method",
            "should_open_predecessor_admission",
            (("impl", "ExactServePredecessorObservation"),),
            "        self.first_target_observation || self.runnable_predecessor\n",
            "        self.first_target_observation && self.runnable_predecessor\n",
            "initial observation or current runnable predecessor alone may open admission",
        ),
    ),
)
def test_exact_serve_direct_identity_mutations_survive_digest_refresh(
    tmp_path: Path,
    item_kind: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Direct completion evidence and observation retain their exact identity."""

    _assert_direct_serve_item_mutation(
        tmp_path,
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        item_kind,
        item_name,
        context,
        old,
        new,
        expected_error,
    )


@pytest.mark.parametrize(
    ("relative", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "exact_serve_predecessor_observation",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "        self.publish_external_lifecycle_owners()?;\n",
            "        let _ = &self.runtime;\n",
            "executor direct predecessor observation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "service_certified_serve_barrier",
            (),
            "            V2IngressDrainMode::CertifiedFenceEscape,\n",
            "            V2IngressDrainMode::Ordinary,\n",
            "ordinary direct selected-Serve predecessor turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "service_pending_certified_serve_barrier",
            (),
            "        if predecessor.has_runnable_predecessor()\n"
            "            && services\n",
            "            advance_executor_once_before_exact_serve(executor, services)?;\n"
            "        if predecessor.has_runnable_predecessor()\n"
            "            && services\n",
            "pending-Kura no-clock Serve turn must not invoke ordinary predecessor work",
        ),
    ),
)
def test_exact_serve_cross_file_boundaries_survive_item_digest_refresh(
    tmp_path: Path,
    relative: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Direct predecessor ownership remains joined across executor and runner files."""

    _assert_direct_serve_item_mutation(
        tmp_path, relative, "method", item_name, context, old, new, expected_error
    )


@pytest.mark.parametrize(
    "mutation",
    (
        "public_field",
        "derive_clone",
        "derive_copy",
        "manual_clone",
        "extra_constructor",
    ),
)
def test_total_checked_gate_rejects_opaque_token_forging(
    tmp_path: Path,
    mutation: str,
) -> None:
    """The authorization token cannot become constructible or duplicable."""

    module = load_checker()
    sources = (
        module._CHECKED_PRODUCTION_TOKEN_SOURCE,
        "crates/iroha_core/src/sumeragi/v2_core.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
    )
    for relative in sources:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)
    path = tmp_path / module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE
    source = path.read_text(encoding="utf-8")
    if mutation == "public_field":
        source = source.replace("    projection: P,", "    pub projection: P,", 1)
    elif mutation == "derive_clone":
        source = source.replace(
            "#[derive(Debug, PartialEq, Eq)]",
            "#[derive(Debug, Clone, PartialEq, Eq)]",
            1,
        )
    elif mutation == "derive_copy":
        source = source.replace(
            "#[derive(Debug, PartialEq, Eq)]",
            "#[derive(Debug, Copy, PartialEq, Eq)]",
            1,
        )
    elif mutation == "manual_clone":
        anchor = "impl<P> CheckedProductionTransition<P> {"
        source = source.replace(
            anchor,
            "impl<P> Clone for CheckedProductionTransition<P> {\n"
            "    fn clone(&self) -> Self { panic!() }\n"
            "}\n\n"
            + anchor,
            1,
        )
    else:
        anchor = "impl<P> CheckedProductionTransition<P> {"
        forged_source = source.replace(
            anchor,
            anchor
            + "\n    fn forge_checked(projection: P) -> Self {\n"
            "        Self {\n"
            "            projection,\n"
            "            first_release_witness: None,\n"
            "        }\n"
            "    }\n",
            1,
        )
        generic_literal_source = source.replace(
            anchor,
            "fn forge_checked_generic() -> "
            "CheckedProductionTransition<[u8; (1 < 2) as usize]> {\n"
            "    CheckedProductionTransition::<[u8; (1 < 2) as usize]> {\n"
            "        projection: [0],\n"
            "        first_release_witness: None,\n"
            "    }\n"
            "}\n\n"
            + anchor,
            1,
        )
        alias_source = source.replace(
            anchor,
            "type Forgeable<P> = CheckedProductionTransition<P>;\n\n"
            "impl<P> Forgeable<P> {\n"
            "    fn forge_checked(projection: P) -> Self {\n"
            "        Self {\n"
            "            projection,\n"
            "            first_release_witness: None,\n"
            "        }\n"
            "    }\n"
            "}\n\n"
            + anchor,
            1,
        )
        grouped_alias_source = source.replace(
            anchor,
            "mod alias_child {\n"
            "    use super::CheckedProductionTransition::{\n"
            "        self, self as Forgeable,\n"
            "    };\n\n"
            "    impl<P> Forgeable<P> {\n"
            "        fn forge_checked(projection: P) -> Self {\n"
            "            Self {\n"
            "                projection,\n"
            "                first_release_witness: None,\n"
            "            }\n"
            "        }\n"
            "    }\n"
            "}\n\n"
            + anchor,
            1,
        )
        macro_alias_source = source.replace(
            anchor,
            "macro_rules! define_forgeable {\n"
            "    ($name:ident, $target:ident) => {\n"
            "        type $name<P> = $target<P>;\n"
            "        impl<P> $name<P> {\n"
            "            fn forge_checked(projection: P) -> Self {\n"
            "                Self {\n"
            "                    projection,\n"
            "                    first_release_witness: None,\n"
            "                }\n"
            "            }\n"
            "        }\n"
            "    };\n"
            "}\n"
            "define_forgeable!(Forgeable, CheckedProductionTransition);\n\n"
            + anchor,
            1,
        )
        helper_source = source.replace(
            "    const fn unwitnessed(projection: P) -> Self {",
            "    pub(crate) const fn unwitnessed(projection: P) -> Self {",
            1,
        )
        path.write_text(helper_source, encoding="utf-8")
        helper_entries = [
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
                "sha256": module._sha256_file(
                    tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
                ),
            },
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
                "sha256": module._sha256_file(path),
            },
        ]
        with pytest.raises(ValueError, match="unwitnessed|constructor|token"):
            module._cross_tool_checked_token_payload(
                source_entries=helper_entries,
                root_dir=tmp_path,
            )
        path.write_text(alias_source, encoding="utf-8")
        alias_entries = [
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
                "sha256": module._sha256_file(
                    tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
                ),
            },
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
                "sha256": module._sha256_file(path),
            },
        ]
        with pytest.raises(ValueError, match="alias|token"):
            module._cross_tool_checked_token_payload(
                source_entries=alias_entries,
                root_dir=tmp_path,
            )
        path.write_text(grouped_alias_source, encoding="utf-8")
        grouped_alias_entries = [
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
                "sha256": module._sha256_file(
                    tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
                ),
            },
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
                "sha256": module._sha256_file(path),
            },
        ]
        with pytest.raises(ValueError, match="alias|token"):
            module._cross_tool_checked_token_payload(
                source_entries=grouped_alias_entries,
                root_dir=tmp_path,
            )
        path.write_text(macro_alias_source, encoding="utf-8")
        macro_alias_entries = [
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
                "sha256": module._sha256_file(
                    tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
                ),
            },
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
                "sha256": module._sha256_file(path),
            },
        ]
        with pytest.raises(ValueError, match="alias|macro|token"):
            module._cross_tool_checked_token_payload(
                source_entries=macro_alias_entries,
                root_dir=tmp_path,
            )
        path.write_text(generic_literal_source, encoding="utf-8")
        generic_entries = [
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
                "sha256": module._sha256_file(
                    tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
                ),
            },
            {
                "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
                "sha256": module._sha256_file(path),
            },
        ]
        with pytest.raises(ValueError, match="literal|token"):
            module._cross_tool_checked_token_payload(
                source_entries=generic_entries,
                root_dir=tmp_path,
            )
        source = forged_source
    path.write_text(source, encoding="utf-8")
    entries = [
        {
            "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
            "sha256": module._sha256_file(
                tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
            ),
        },
        {
            "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
            "sha256": module._sha256_file(path),
        },
    ]
    with pytest.raises(ValueError, match="token|CheckedProductionTransition"):
        module._cross_tool_checked_token_payload(
            source_entries=entries,
            root_dir=tmp_path,
        )


@pytest.mark.parametrize(
    "mutation",
    (
        "in_flight_projection_visibility",
        "in_flight_macro_predicate",
        "in_flight_kernel_body",
        "in_flight_constructor_always_some",
        "in_flight_constructor_wrong_kernel",
        "materialization_projection_visibility",
        "legacy_constructor_always_some",
        "borrower_visibility",
        "borrower_body",
    ),
)
def test_total_checked_gate_rejects_in_flight_token_contract_weakening(
    tmp_path: Path,
    mutation: str,
) -> None:
    """The auxiliary reservation gate cannot weaken the shared opaque token."""

    module = load_checker()
    sources = (
        module._CHECKED_PRODUCTION_TOKEN_SOURCE,
        "crates/iroha_core/src/sumeragi/v2_core.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
    )
    for relative in sources:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)
    path = tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
    source = path.read_text(encoding="utf-8")

    if mutation == "in_flight_projection_visibility":
        item = module.rust_struct_items(
            source, "ProductionInFlightReservationTransitionProjection"
        )[0]
        assert item.source.count("    pub(crate) action: u8,") == 1
        mutated = item.source.replace(
            "    pub(crate) action: u8,",
            "    pub action: u8,",
            1,
        )
    elif mutation == "in_flight_macro_predicate":
        item = module.rust_macro_items(
            source, "production_in_flight_reservation_transition_body"
        )[0]
        old = (
            "projection.before.state "
            "== refinement_tag_value!(IN_FLIGHT_RESERVATION_STATE_ABSENT)"
        )
        new = "true"
        assert item.source.count(old) >= 1
        mutated = item.source.replace(old, new, 1)
    elif mutation == "in_flight_kernel_body":
        item = module.rust_items(
            source, "production_in_flight_reservation_transition_kernel"
        )[0]
        old = "production_in_flight_reservation_transition_body!(projection)"
        new = "projection.action > 0"
        assert item.source.count(old) == 1
        mutated = item.source.replace(old, new, 1)
    elif mutation.startswith("in_flight_constructor_"):
        item = module.rust_items(
            source, "check_production_in_flight_reservation_transition"
        )[0]
        if mutation == "in_flight_constructor_always_some":
            old = "    } else {\n        None\n    }"
            new = (
                "    } else {\n"
                "        Some(CheckedProductionTransition::unwitnessed(projection))\n"
                "    }"
            )
        else:
            old = "production_in_flight_reservation_transition_kernel(projection)"
            new = (
                "production_application_trace_refines_decision_completion_kernel("
                "Default::default())"
            )
        assert item.source.count(old) == 1
        mutated = item.source.replace(old, new, 1)
    elif mutation == "materialization_projection_visibility":
        item = module.rust_struct_items(
            source, "ProductionIngressReservationMaterializationTraceProjection"
        )[0]
        old = "    pub(crate) reserved_slots_before: u8,"
        new = "    pub reserved_slots_before: u8,"
        assert item.source.count(old) == 1
        mutated = item.source.replace(old, new, 1)
    elif mutation == "legacy_constructor_always_some":
        item = module.rust_items(
            source,
            "check_production_body_ownership_effective_lock_transition",
        )[0]
        old = "    } else {\n        None\n    }"
        new = (
            "    } else {\n"
            "        Some(CheckedProductionTransition::unwitnessed(projection))\n"
            "    }"
        )
        assert item.source.count(old) == 1
        mutated = item.source.replace(old, new, 1)
    else:
        path = tmp_path / module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE
        source = path.read_text(encoding="utf-8")
        item = module.rust_items(source, "accepted_projection")[0]
        if mutation == "borrower_visibility":
            old = "pub(crate) const fn accepted_projection"
            new = "pub const fn accepted_projection"
        else:
            old = "&self.projection"
            new = "match self { Self { projection } => projection }"
        assert item.source.count(old) == 1
        mutated = item.source.replace(old, new, 1)

    mutated_source = source.replace(item.source, mutated, 1)
    if mutation == "borrower_body":
        mutated_source = mutated_source.replace(
            "            first_release_witness: None,",
            "            first_release_witness: panic!(),",
            1,
        )
    path.write_text(mutated_source, encoding="utf-8")
    entries = [
        {
            "path": module._CHECKED_PRODUCTION_TOKEN_SOURCE,
            "sha256": module._sha256_file(
                tmp_path / module._CHECKED_PRODUCTION_TOKEN_SOURCE
            ),
        },
        {
            "path": module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE,
            "sha256": module._sha256_file(
                tmp_path / module._CHECKED_PRODUCTION_TOKEN_DEFINITION_SOURCE
            ),
        },
    ]
    with pytest.raises(
        ValueError,
        match="in-flight|materialization|effective-lock|borrowed|borrower|token",
    ):
        module._cross_tool_checked_token_payload(
            source_entries=entries,
            root_dir=tmp_path,
        )



@pytest.mark.parametrize(
    ("relative", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert!(!suppressed.should_open_predecessor_admission());\n",
            "        assert!(suppressed.should_open_predecessor_admission());\n",
            "runtime direct-observation retry regression",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "late_passive_fetch_completion_opens_one_serve_predecessor_admission_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert!(initial.should_open_predecessor_admission());\n",
            "        assert!(!initial.should_open_predecessor_admission());\n",
            "late passive Fetch direct-observation regression",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker_io_and_selected_serve_cases_01_tests.rs",
            "dropping_exact_serve_predecessor_admission_closes_transient_aperture",
            (),
            "    drop(predecessor_admission);\n",
            "    std::mem::forget(predecessor_admission);\n",
            "move-only guard Drop regression",
        ),
    ),
)
def test_direct_serve_predecessor_regressions_survive_item_digest_refresh(
    tmp_path: Path,
    relative: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Direct-observation and guard-Drop regressions survive structural resealing."""

    _assert_direct_serve_item_mutation(
        tmp_path, relative, "method", item_name, context, old, new, expected_error
    )
