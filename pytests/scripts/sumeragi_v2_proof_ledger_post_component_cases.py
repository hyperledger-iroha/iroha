"""Late-bound proof-ledger cases executed in the canonical test namespace."""

def test_test_only_height_ingress_wrappers_are_not_production_seals() -> None:
    """Only current atomic retirement seams belong to the production inventory."""

    module = load_checker()
    assert set(module._PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256) == {
        "runner::close_ingress_for_rollover",
        "ingress::retire_leader_wire_lifecycle_gate",
        "leader_wire_store::park_sealed_ingress",
        "ingress::close",
    }
    assert not hasattr(
        module,
        "_PRODUCTION_HEIGHT_INGRESS_BINDING_TEST_ITEM_SHA256",
    )
    assert not hasattr(
        module,
        "_PRODUCTION_CERTIFIED_SERVE_INGRESS_BINDING_ITEM_SHA256",
    )


def test_apply_terminal_direct_broadcast_exact_one_seams_are_source_sealed() -> None:
    """The move-only authority and both exact settlers fail closed on seal drift."""

    module = load_checker()
    direct_output_seams = {
        "height::completion_selection_retries_before_runtime",
        "scheduler::ProductionLifecycleOwnerV1::wake_apply_terminal_direct_broadcast_if_fenced",
        "scheduler::ProductionLifecycleOwnerV1::prepare_apply_terminal_direct_broadcast",
        "registry::PreparedApplyTerminalDirectBroadcastV1",
        "registry::ConcreteLifecycleWorkRegistry::prepare_apply_terminal_direct_broadcast",
        "registry::ConcreteLifecycleWorkRegistry::apply_terminal_direct_broadcast_pending_is_exact",
        "admission::ProductionLifecycleOwnerV1::settle_apply_terminal_direct_broadcast",
        "effects::V2EffectExecutor::settle_apply_terminal_direct_broadcast",
    }
    inventory = module._PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256
    assert direct_output_seams <= set(inventory)
    assert (
        module._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(
            ROOT_DIR
        )
        == []
    )

    stale_digest = "0" * 64
    for seam in sorted(direct_output_seams):
        reviewed_digest = inventory[seam]
        inventory[seam] = stale_digest
        try:
            errors = (
                module._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(
                    ROOT_DIR
                )
            )
        finally:
            inventory[seam] = reviewed_digest
        assert any(
            stale_digest in error and "exact reviewed token digest" in error
            for error in errors
        ), (seam, errors)


@pytest.mark.parametrize(
    (
        "digest_key",
        "relative_path",
        "item_name",
        "brace_context",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "runner::close_ingress_for_rollover",
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "close_ingress_for_rollover",
            (),
            "ingress_ready.store(false, Ordering::Release);\n    block_ingress.close();",
            "block_ingress.close();\n    ingress_ready.store(false, Ordering::Release);",
            "rollover close must publish not-ready before closing",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "let _service_guard = self.service_lock.lock();",
            "let _service_guard = self.state.lock();",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "state.open = false;",
            "state.open = true;",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "if !serviced_candidate_store::LeaderWireLifecycleStoreGate::ptr_eq(&bound, gate) {",
            "if false {",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "if carriers != mirrored_ingress {",
            "if false {",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "let retirement = bound.park_sealed_ingress(carriers)?;",
            "let retirement = bound.park_sealed_ingress(BTreeMap::new())?;",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "state.leader_wire_lifecycle_gate = None;",
            "let _ = state.leader_wire_lifecycle_gate.as_ref();",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "ingress::retire_leader_wire_lifecycle_gate",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "retire_leader_wire_lifecycle_gate",
            (("impl", "FairV2Ingress"),),
            "retirement.complete();",
            "drop(retirement);",
            "atomic leader-wire retirement must exclude consumers and producers",
        ),
        (
            "leader_wire_store::park_sealed_ingress",
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "park_sealed_ingress",
            (("impl", "LeaderWireLifecycleStoreGate"),),
            "slot != &token.slot",
            "slot == &token.slot",
            "durable leader-wire retirement must validate the exact Ingress set",
        ),
        (
            "leader_wire_store::park_sealed_ingress",
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "park_sealed_ingress",
            (("impl", "LeaderWireLifecycleStoreGate"),),
            "if durable_ingress != carriers",
            "if durable_ingress == carriers",
            "durable leader-wire retirement must validate the exact Ingress set",
        ),
        (
            "leader_wire_store::park_sealed_ingress",
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "park_sealed_ingress",
            (("impl", "LeaderWireLifecycleStoreGate"),),
            "|| record.runtime_owner.is_some()",
            "|| record.runtime_owner.is_none()",
            "durable leader-wire retirement must validate the exact Ingress set",
        ),
        (
            "leader_wire_store::park_sealed_ingress",
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "park_sealed_ingress",
            (("impl", "LeaderWireLifecycleStoreGate"),),
            "*state = previous;\n            return Err(error);",
            "return Err(error);\n            *state = previous;",
            "durable leader-wire retirement must validate the exact Ingress set",
        ),
        (
            "leader_wire_store::park_sealed_ingress",
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            "park_sealed_ingress",
            (("impl", "LeaderWireLifecycleStoreGate"),),
            "Ok(SealedLeaderWireIngressRetirementV1 { _private: () })",
            "Err(\"retirement receipt lost\".to_owned())",
            "durable leader-wire retirement must validate the exact Ingress set",
        ),
        (
            "ingress::close",
            "crates/iroha_core/src/sumeragi/mod.rs",
            "close",
            (("impl", "FairV2Ingress"),),
            "self.state.lock().open = false;",
            "self.state.lock().open = true;",
            "fair ingress close must make admission unavailable",
        ),
    ),
)
def test_leader_wire_height_ingress_semantics_survive_pending_digest_refresh(
    tmp_path: Path,
    digest_key: str,
    relative_path: str,
    item_name: str,
    brace_context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Each leader-wire production retirement seam survives a refreshed digest."""

    module = load_checker()
    exact_output_production_fixture(tmp_path)
    path = tmp_path / relative_path
    mutate_rust_item_source_in_context(
        module,
        path,
        item_name,
        brace_context,
        old,
        new,
    )
    items = [
        item
        for item in module.rust_items(path.read_text(encoding="utf-8"), item_name)
        if item.brace_context == brace_context
    ]
    assert len(items) == 1, digest_key
    module._PRODUCTION_HEIGHT_INGRESS_BINDING_ITEM_SHA256[digest_key] = (
        module._rust_item_token_sha256(items[0])
    )

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)

    assert any(expected_error in error for error in errors), errors




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
        Path("crates/iroha_core/src/sumeragi/fair_v2_ingress_selector.rs"),
        Path(
            "crates/iroha_core/src/sumeragi/tests/"
            "mod_authoritative_runtime_gate_03_admission_and_fairness.rs"
        ),
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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
        "crates/iroha_core/src/sumeragi/fair_v2_ingress_selector.rs"
    )
    missing_component = tmp_path / missing_relative
    canonical_component = ROOT_DIR / missing_relative
    assert missing_component.is_file() and not missing_component.is_symlink()
    assert missing_component.read_bytes() == canonical_component.read_bytes()
    missing_component.unlink()
    missing_errors = module._timeout_vote_episode_source_fidelity_errors(
        tmp_path, formal_dir
    )
    assert any(
        f"{missing_component}: timeout-vote episode selector source must be a regular file"
        in error
        for error in missing_errors
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
        "strict candidates must remain ahead of all dependency candidates"
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
    path = reviewed_rust_item_provider(
        module, repo_root, relative, item_name
    )
    items = module.rust_items(path.read_text(encoding="utf-8"), item_name)
    assert len(items) == 1, (relative, item_name)
    digest = module._rust_item_token_sha256(items[0])
    rebound: list[str] = []
    role_relatives = {
        "ingress": Path("crates/iroha_core/src/sumeragi/mod.rs"),
        "runtime": Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
    }
    for key in module._TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256:
        role, qualified_name = key.split("::", 1)
        role_relative = role_relatives[role]
        if qualified_name == "fair_v2_ingress_queue_gate_verdict":
            role_relative = Path(
                "crates/iroha_core/src/sumeragi/fair_v2_ingress_selector.rs"
            )
        if (
            role_relative == relative
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
            Path(
                "crates/iroha_core/src/sumeragi/tests/"
                "mod_authoritative_runtime_gate_03_admission_and_fairness.rs"
            ),
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
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs"),
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
        Path("scripts/verify_sumeragi_v2.sh"),
        Path("configs/soranexus/taira/config.toml"),
        Path("configs/soranexus/taira/genesis.json"),
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
        Path("crates/iroha_core/src/sumeragi/serviced_candidate_store.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_body_store.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_authority.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs"),
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs"),
        Path("crates/iroha_core/src/sumeragi/tests/v2_lifecycle_scheduler_certified_serve_cases.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/actual/tests.rs"),
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
    # The lifecycle registry's reviewed include closure has one nested layer.
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

    fixture_root = tmp_path / "boundary_mutations"
    exact_output_production_fixture(fixture_root)
    mutations = (
        (
            "crates/iroha_config/src/parameters/actual.rs",
            ".and_then(|observer| validator_roster_len.checked_add(observer))",
            ".and_then(|observer| MAX_VALIDATORS_PER_HEIGHT.checked_add(observer))",
            "exact validator roster",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            ".max(1)\n        .checked_mul(certified_request_capacity)",
            ".max(2)\n        .checked_mul(certified_request_capacity)",
            "authenticated-source lower boundary",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            ".checked_mul(certified_request_capacity)",
            ".saturating_mul(certified_request_capacity)",
            "checked authenticated-source multiplication",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            ".and_then(|sum| sum.checked_add(producer))",
            ".and_then(|sum| Some(sum.saturating_add(producer)))",
            "checked aggregate arithmetic",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "if total > maximum {",
            "if total >= maximum {",
            "aggregate total boundary",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT: usize = u16::MAX as usize + 1;",
            "pub const V2_MAX_LIFECYCLE_RECORDS_PER_HEIGHT: usize = u16::MAX as usize;",
            "65,536-record physical namespace",
        ),
    )
    expected_errors = []
    for relative, old, new, expected_error in mutations:
        path = fixture_root / relative
        source = path.read_text(encoding="utf-8")
        assert source.count(old) == 1, (relative, old)
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        expected_errors.append(expected_error)

    errors = module._lifecycle_capacity_production_source_fidelity_errors(fixture_root)
    for expected_error in expected_errors:
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
        / "autonomous_lane_output_reconstruction.rs"
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
    item_name = "applied_height_handoff_retires_only_exact_same_finality_nonwinning_autonomous_outputs_atomically"
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
        ((module._APPLIED_HEIGHT_PREDECESSOR_DURABILITY_HANDOFF_TEST_SHA256, item_name),),
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
        path = (
            tmp_path
            / "crates"
            / "iroha_core"
            / "src"
            / "sumeragi"
            / "v2_core"
            / "refinement"
            / "post_carrier_transition.rs"
        )
        source = path.read_text(encoding="utf-8")
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
    ("old", "new", "expected_error"),
    (
        (
            "let redispatch = if runtime_terminal_incumbent {\n"
            "                        false",
            "let redispatch = if runtime_terminal_incumbent {\n"
            "                        true",
            "runtime-owned terminals stutter",
        ),
        (
            "                    } else if matches!(\n"
            "                        fetch_authority_relation,\n"
            "                        Some(RuntimeFetchAuthorityRelation::Stale)\n"
            "                    ) {",
            "                    } else if false && matches!(\n"
            "                        fetch_authority_relation,\n"
            "                        Some(RuntimeFetchAuthorityRelation::Stale)\n"
            "                    ) {",
            "stale Fetch authority stutter",
        ),
        (
            ".adopt_incumbent_fetch_for_retry_or_authority(evidence, effect)",
            ".adopt_incumbent_body_stage_for_retry_or_authority(evidence, effect)",
            "Fetch authority upgrades must adopt, re-prove, and publish",
        ),
        (
            ".adopt_incumbent_body_stage_for_retry_or_authority(evidence, effect)",
            ".adopt_incumbent_fetch_for_retry_or_authority(evidence, effect)",
            "Store and Validate authority upgrades must adopt and re-prove",
        ),
        (
            "} else if matches!(\n"
            "                        fetch_authority_relation,\n"
            "                        Some(RuntimeFetchAuthorityRelation::Stale)\n"
            "                    ) {",
            "} else if matches!(\n"
            "                        fetch_authority_relation,\n"
            "                        Some(RuntimeFetchAuthorityRelation::Same)\n"
            "                    ) {",
            "stale Fetch authority stutters",
        ),
    ),
)
def test_retained_candidate_retry_semantics_survive_item_digest_refresh(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Refreshing the FIFO item seal cannot hide retry-owner drift."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    effects_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    item_name = "retain_effect_batch_at_frontier"
    mutate_rust_item_source(module, effects_path, item_name, old, new)
    items = module.rust_items(effects_path.read_text(encoding="utf-8"), item_name)
    assert len(items) == 1
    module._PRODUCTION_RETAINED_EFFECT_FIFO_ITEM_SHA256[item_name] = (
        module._rust_item_token_sha256(items[0])
    )

    errors = module._effect_capacity_production_source_fidelity_errors(tmp_path)

    assert any(expected_error in error for error in errors), errors
