"""Late-bound proof-ledger cases executed in the canonical test namespace."""

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
        Path("crates/iroha_core/src/sumeragi/v2_runner_tests.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner/height_ingress_bindings.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)


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
    """Rebind only the include-expanded source changed by one mutation."""

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
    assert len(rebound_relatives) == 1, rebound_relatives
    errors = module._same_round_semantic_kernel_source_fidelity_errors(repo_root)
    assert not any(
        "same-round semantic kernel source must match exact reviewed SHA-256"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    (
        "seal_group",
        "seal_key",
        "item_kind",
        "owner",
        "item_name",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256",
            "V2IoCertifiedServeIngressReservation",
            "struct",
            "",
            "V2IoCertifiedServeIngressReservation",
            "    lifecycle_id: CertifiedServeLifecycleId,\n",
            "    lifecycle_id: CertifiedServeIngressReservationId,\n",
            "logical lifecycle, payload, carrier, bounded runtime turn, and last "
            "consumed predecessor witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256",
            "V2IoCertifiedServeIngressReservation",
            "struct",
            "",
            "V2IoCertifiedServeIngressReservation",
            "    last_predecessor_episode_witness: Option<ExactServePredecessorEpisodeWitness>,\n",
            "    last_predecessor_episode_witness: bool,\n",
            "logical lifecycle, payload, carrier, bounded runtime turn, and last "
            "consumed predecessor witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256",
            "V2IoCommandQueueState",
            "struct",
            "",
            "V2IoCommandQueueState",
            "    producer_episode_due: bool,\n",
            "    producer_episode_due: u8,\n",
            "distinct one-shot due and finite active producer-episode fields",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256",
            "V2IoCommandQueueState",
            "struct",
            "",
            "V2IoCommandQueueState",
            "    producer_episode_active: bool,\n",
            "    producer_episode_active: u8,\n",
            "distinct one-shot due and finite active producer-episode fields",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256",
            "barrier",
            "method",
            "V2IoCertifiedServeIngressReservation",
            "barrier",
            "            scheduler_ordinal: self.id.0,\n"
            "            lifecycle_id: self.lifecycle_id,\n",
            "            scheduler_ordinal: self.lifecycle_id.admission_ordinal,\n"
            "            lifecycle_id: self.lifecycle_id,\n",
            "physical scheduler ticket, logical lifecycle, and carrier",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256",
            "barrier",
            "method",
            "V2IoCertifiedServeIngressReservation",
            "barrier",
            "            lifecycle_id: self.lifecycle_id,\n",
            "            lifecycle_id: CertifiedServeLifecycleId {\n"
            "                admission_ordinal: self.id.0,\n"
            "                request_hash: self.projection.request_hash,\n"
            "            },\n",
            "physical scheduler ticket, logical lifecycle, and carrier",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256",
            "matches_barrier",
            "method",
            "V2IoCertifiedServeIngressReservation",
            "matches_barrier",
            "self.id.0 == barrier.scheduler_ordinal",
            "self.lifecycle_id.admission_ordinal == barrier.scheduler_ordinal",
            "exact physical Serve scheduler ticket",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_RESERVATION_ITEM_SHA256",
            "matches_barrier",
            "method",
            "V2IoCertifiedServeIngressReservation",
            "matches_barrier",
            "&& self.lifecycle_id == barrier.lifecycle_id",
            "&& self.lifecycle_id.request_hash == barrier.lifecycle_id.request_hash",
            "exact logical Serve lifecycle",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::suspend_materialized_serve_barrier_for_runtime_predecessor",
            "method",
            "V2IoCommandQueue",
            "suspend_materialized_serve_barrier_for_runtime_predecessor",
            "            index + 1,\n            state.commands.len(),\n",
            "            index,\n            state.commands.len(),\n",
            "only the physical FIFO-tail target unit may transfer to an older owner",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::claim_serve_runtime_episode",
            "method",
            "V2IoCommandQueue",
            "claim_serve_runtime_episode",
            "            | CertifiedServeRuntimeEpisodeState::Complete => Ok(false),\n",
            "            | CertifiedServeRuntimeEpisodeState::Complete => Ok(true),\n",
            "one exact occurrence may claim only one unsettled predecessor turn",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::reserve_serve_ingress",
            "method",
            "V2IoCommandQueue",
            "reserve_serve_ingress",
            "            last_predecessor_episode_witness: None,\n",
            "            last_predecessor_episode_witness: Some(\n"
            "                ExactServePredecessorEpisodeWitness::for_test(3, 1, 1),\n"
            "            ),\n",
            "must start ready with no consumed predecessor witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::reserve_serve_ingress",
            "method",
            "V2IoCommandQueue",
            "reserve_serve_ingress",
            "        let ordinal = match self.lifecycle_ordinals.reserve_one() {\n",
            "        let ordinal = match Ok(\n"
            "            state.next_serve_ingress_reservation_ordinal.saturating_add(1),\n"
            "        ) {\n",
            "exact-Serve tickets must use a fresh actor-global monotone ordinal",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::reserve_serve_ingress",
            "method",
            "V2IoCommandQueue",
            "reserve_serve_ingress",
            "            runtime_episode: CertifiedServeRuntimeEpisodeState::Ready,\n",
            "            runtime_episode: CertifiedServeRuntimeEpisodeState::Complete,\n",
            "must start ready with no consumed predecessor witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "            if witness.episode() != expected_episode {\n",
            "            if witness.episode() < expected_episode {\n",
            "must advance by exactly one checked consumer episode",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "                if witness != previous {\n",
            "                if witness == previous {\n",
            "conflicting or regressing evidence fails closed",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "                return Ok(false);\n",
            "                return Ok(true);\n",
            "repeated witness must stutter",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "            reservation.runtime_episode = CertifiedServeRuntimeEpisodeState::Ready;\n",
            "            reservation.runtime_episode = CertifiedServeRuntimeEpisodeState::Complete;\n",
            "newly consumed witness may reopen a sealed Complete target to Ready",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "            || witness.serve_lifecycle_ordinal() != barrier.scheduler_ordinal()\n",
            "            || witness.serve_lifecycle_ordinal() == barrier.scheduler_ordinal()\n",
            "must validate the exact target and strict predecessor before consuming a witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::observe_serve_predecessor_episode_witness",
            "method",
            "V2IoCommandQueue",
            "observe_serve_predecessor_episode_witness",
            "        } else if witness.episode() != 1 {\n",
            "        } else if witness.episode() != 0 {\n",
            "the first consumed predecessor witness must begin at one and become immutable reservation evidence",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::serve_runtime_predecessor_capacity_available",
            "method",
            "V2IoCommandQueue",
            "serve_runtime_predecessor_capacity_available",
            "        Ok(transferable_target_slot\n"
            "            || (state.commands.len() < self.capacity\n"
            "                && self.admission.has_capacity(V2IoAdmissionClass::Consensus)))\n",
            "        Ok(transferable_target_slot\n"
            "            && (state.commands.len() < self.capacity\n"
            "                && self.admission.has_capacity(V2IoAdmissionClass::Consensus)))\n",
            "atomically transferable target unit",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::finish_serve_runtime_episode_turn",
            "method",
            "V2IoCommandQueue",
            "finish_serve_runtime_episode_turn",
            "        reservation.runtime_episode = if older_predecessor_remains {\n"
            "            CertifiedServeRuntimeEpisodeState::Ready\n"
            "        } else {\n"
            "            CertifiedServeRuntimeEpisodeState::Complete\n"
            "        };\n",
            "        reservation.runtime_episode = if older_predecessor_remains {\n"
            "            CertifiedServeRuntimeEpisodeState::Complete\n"
            "        } else {\n"
            "            CertifiedServeRuntimeEpisodeState::Ready\n"
            "        };\n",
            "mandatory full recheck must either reopen one bounded turn or seal the occurrence",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::serve_barrier",
            "method",
            "V2IoCommandQueue",
            "serve_barrier",
            "        if barrier.request_hash != lifecycle_id.request_hash || barrier.lifecycle_id != lifecycle_id\n",
            "        if barrier.request_hash != lifecycle_id.request_hash\n",
            "validate both request and logical lifecycle",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::serve_barrier",
            "method",
            "V2IoCommandQueue",
            "serve_barrier",
            "                || !state.serves.contains_key(&reservation.lifecycle_id))\n",
            "                || false)\n",
            "raw exact-Serve barrier must remain indexed by its immutable logical lifecycle",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "V2IoCommandQueue::try_send_as",
            "method",
            "V2IoCommandQueue",
            "try_send_as",
            "        let exact_target_active = state.serve_ingress_reservation.is_some()\n"
            "            || !state.serve_ingress_waiters.is_empty()\n"
            "            || state.serve_barrier.is_some();\n",
            "        let exact_target_active = state.serve_ingress_reservation.is_some()\n"
            "            || state.serve_barrier.is_some();\n",
            "selected target, any admitted waiter, or its materialized barrier",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_barrier",
            "method",
            "ProductionV2Services",
            "certified_serve_barrier",
            "        self.io.as_ref().map_or(Ok(None), V2IoHandle::serve_barrier)\n",
            "        Ok(None)\n",
            "production exact-Serve barrier wrapper must project only through the attached I/O owner",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::claim_certified_serve_runtime_episode",
            "method",
            "ProductionV2Services",
            "claim_certified_serve_runtime_episode",
            "            .claim_serve_runtime_episode(barrier)\n",
            "            .serve_runtime_predecessor_capacity_available(barrier)\n",
            "production exact-Serve claim wrapper must fail closed and forward the exact barrier",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::observe_certified_serve_predecessor_episode_witness",
            "method",
            "ProductionV2Services",
            "observe_certified_serve_predecessor_episode_witness",
            "            .observe_serve_predecessor_episode_witness(barrier, witness)\n",
            "            .claim_serve_runtime_episode(barrier)\n",
            "production predecessor-witness wrapper must fail closed and forward the exact barrier and witness",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_runtime_predecessor_capacity_available",
            "method",
            "ProductionV2Services",
            "certified_serve_runtime_predecessor_capacity_available",
            "            .serve_runtime_predecessor_capacity_available(barrier)\n",
            "            .claim_serve_runtime_episode(barrier)\n",
            "production exact-Serve capacity wrapper must fail closed and forward the exact barrier",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::finish_certified_serve_runtime_episode_turn",
            "method",
            "ProductionV2Services",
            "finish_certified_serve_runtime_episode_turn",
            "            .finish_serve_runtime_episode_turn(barrier, older_predecessor_remains)\n",
            "            .finish_serve_runtime_episode_turn(barrier, false)\n",
            "production exact-Serve settlement wrapper must fail closed and forward the barrier and recheck result",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::try_begin_certified_serve_producer_episode",
            "method",
            "ProductionV2Services",
            "try_begin_certified_serve_producer_episode",
            "            .try_begin_producer_episode()\n",
            "            .try_begin_producer_episode().map(|_| None)\n",
            "production producer-episode wrapper must fail closed and delegate to the queue-atomic gate",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::take_exact_serve_predecessor_completion",
            "method",
            "ProductionV2Services",
            "take_exact_serve_predecessor_completion",
            "            serve_lifecycle_ordinal,\n            false,\n",
            "            serve_lifecycle_ordinal,\n            true,\n",
            "shared helper with a strict lifecycle cut",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::take_lifecycle_prefix_completion",
            "method",
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "                ordinal < lifecycle_cut\n",
            "                ordinal <= lifecycle_cut\n",
            "distinguish inclusive timeout ownership from strict exact-Serve predecessors",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::take_lifecycle_prefix_completion",
            "method",
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "                    within_cut(ordinal)\n"
            "                        && (runtime_capacity_available || !owned.requires_runtime_capacity)\n",
            "                    within_cut(ordinal)\n",
            "reviewed lifecycle cut and runtime-capacity gate",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::take_lifecycle_prefix_completion",
            "method",
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "                .min_by_key(|completion| completion.runtime_lifecycle_ordinal())\n",
            "                .max_by_key(|completion| completion.runtime_lifecycle_ordinal())\n",
            "choose the least immutable ordinal",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::take_lifecycle_prefix_completion",
            "method",
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "            (Some(io), Some(local)) if io < local => Some(CompletionSource::Io),\n",
            "            (Some(io), Some(local)) if io > local => Some(CompletionSource::Io),\n",
            "choose the least owner and retain finite fair tie-breaking",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_exact_serve_runtime_predecessor",
            "method",
            "ProductionV2Services",
            "drain_exact_serve_runtime_predecessor",
            "            CompletionDrainPolicy::ExactServePredecessor {\n"
            "                serve_lifecycle_ordinal,\n"
            "            },\n",
            "            CompletionDrainPolicy::TimeoutRecoveryPrefix {\n"
            "                inclusive_lifecycle_cut: serve_lifecycle_ordinal,\n"
            "            },\n",
            "at most one completed owner",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            "        while attempts < limit {\n",
            "        while attempts <= limit {\n",
            "caller-supplied finite bound",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            "                } => self.take_exact_serve_predecessor_completion(\n",
            "                } => self.take_timeout_recovery_prefix_completion(\n",
            "exact policy must use only the strict ticket-indexed selector",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            "                } => self.take_timeout_recovery_prefix_completion(\n",
            "                } => self.take_exact_serve_predecessor_completion(\n",
            "separately inclusive timeout-recovery selector",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            "if disposition == CompletionDisposition::Accepted {",
            "if disposition == CompletionDisposition::Rejected {",
            "only an accepted application completion may refresh the exact durable Kura tip and refresh failures must fail closed",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            "Some((source_height, source_block_hash)),",
            "None,",
            "only an accepted application completion may refresh the exact durable Kura tip and refresh failures must fail closed",
        ),
        (
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::drain_completions_inner",
            "method",
            "ProductionV2Services",
            "drain_completions_inner",
            ".map_err(|reason| executor.external_service_failed(reason, self))?;",
            ".map_err(|reason| executor.external_service_failed(reason, self));",
            "only an accepted application completion may refresh the exact durable Kura tip and refresh failures must fail closed",
        ),
    ),
)
def test_exact_serve_checker_boundaries_survive_item_digest_refresh(
    tmp_path: Path,
    seal_group: str,
    seal_key: str,
    item_kind: str,
    owner: str,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Each exact-Serve item keeps a semantic check after resealing itself."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        items = module.rust_struct_items(source, item_name)
    else:
        context = (("impl", owner),)
        items = tuple(
            item
            for item in module.rust_items(source, item_name)
            if item.brace_context == context
        )
    assert len(items) == 1, (item_name, [item.brace_context for item in items])
    item = items[0]
    assert item.source.count(old) == 1, (seal_key, old)
    mutated_source = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1, seal_key
    path.write_text(
        source.replace(item.source, mutated_source, 1),
        encoding="utf-8",
    )

    mutated_file = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        mutated_items = module.rust_struct_items(mutated_file, item_name)
    else:
        mutated_items = tuple(
            candidate
            for candidate in module.rust_items(mutated_file, item_name)
            if candidate.brace_context == (("impl", owner),)
        )
    assert len(mutated_items) == 1
    getattr(module, seal_group)[seal_key] = module._rust_item_token_sha256(
        mutated_items[0]
    )
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_kind", "seal_group", "seal_key", "item_name", "old", "new", "expected_error"),
    (
        (
            "struct",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256",
            "ExactServePredecessorCompletionEvidence",
            "ExactServePredecessorCompletionEvidence",
            "    lifecycle_ordinal_complement: u128,\n",
            "    lifecycle_ordinal_checksum: u128,\n",
            "completion evidence must bind one immutable ordinal and its exact integrity complement",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorCompletionEvidence::try_new",
            "try_new",
            "            lifecycle_ordinal_complement: !lifecycle_ordinal,\n",
            "            lifecycle_ordinal_complement: lifecycle_ordinal,\n",
            "completion-evidence construction must derive its exact integrity complement",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorCompletionEvidence::validate_exact",
            "validate_exact",
            "        self.lifecycle_ordinal > 0 && self.lifecycle_ordinal_complement == !self.lifecycle_ordinal\n",
            "        self.lifecycle_ordinal > 0 && true\n",
            "completion evidence must reject zero or a mismatched integrity complement",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorCompletionEvidence::lifecycle_ordinal",
            "lifecycle_ordinal",
            "        self.lifecycle_ordinal\n",
            "        self.lifecycle_ordinal_complement\n",
            "completion evidence must project exactly its validated lifecycle ordinal",
        ),
        (
            "struct",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256",
            "ExactServePredecessorEpisodeWitness",
            "ExactServePredecessorEpisodeWitness",
            "    serve_lifecycle_ordinal: u128,\n",
            "    serve_target: u128,\n",
            "witness must bind the immutable target, strict predecessor, and monotone episode",
        ),
        (
            "struct",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256",
            "ExactServePredecessorEpisodeWitness",
            "ExactServePredecessorEpisodeWitness",
            "    predecessor_lifecycle_ordinal: u128,\n",
            "    predecessor_target: u128,\n",
            "witness must bind the immutable target, strict predecessor, and monotone episode",
        ),
        (
            "struct",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_STRUCT_SHA256",
            "ExactServePredecessorEpisodeWitness",
            "ExactServePredecessorEpisodeWitness",
            "    episode: u128,\n",
            "    episode_sequence: u128,\n",
            "witness must bind the immutable target, strict predecessor, and monotone episode",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorEpisodeWitness::try_new",
            "try_new",
            "        witness.validate_exact().then_some(witness)\n",
            "        Some(witness)\n",
            "witness construction must validate the complete immutable evidence before publication",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorEpisodeWitness::validate_exact",
            "validate_exact",
            "            && self.predecessor_lifecycle_ordinal < self.serve_lifecycle_ordinal\n",
            "            && self.predecessor_lifecycle_ordinal <= self.serve_lifecycle_ordinal\n",
            "witness validation must require nonzero target, strict nonzero predecessor, and nonzero episode",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorEpisodeWitness::validate_exact",
            "validate_exact",
            "        self.serve_lifecycle_ordinal > 0\n",
            "        true\n",
            "witness validation must require nonzero target, strict nonzero predecessor, and nonzero episode",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorEpisodeWitness::validate_exact",
            "validate_exact",
            "            && self.predecessor_lifecycle_ordinal > 0\n",
            "            && true\n",
            "witness validation must require nonzero target, strict nonzero predecessor, and nonzero episode",
        ),
        (
            "method",
            "_EXACT_SERVE_PREDECESSOR_WITNESS_ITEM_SHA256",
            "ExactServePredecessorEpisodeWitness::validate_exact",
            "validate_exact",
            "            && self.episode > 0\n",
            "            && true\n",
            "witness validation must require nonzero target, strict nonzero predecessor, and nonzero episode",
        ),
    ),
)
def test_exact_serve_witness_identity_mutations_survive_digest_refresh(
    tmp_path: Path,
    item_kind: str,
    seal_group: str,
    seal_key: str,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Witness identity and validation stay semantic after individual reseal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        items = module.rust_struct_items(source, item_name)
    else:
        owner = seal_key.rsplit("::", 1)[0]
        items = tuple(
            item
            for item in module.rust_items(source, item_name)
            if item.brace_context
            == (("impl", owner),)
        )
    assert len(items) == 1
    item = items[0]
    assert item.source.count(old) == 1, (seal_key, old)
    path.write_text(
        source.replace(item.source, item.source.replace(old, new, 1), 1),
        encoding="utf-8",
    )
    mutated_source = path.read_text(encoding="utf-8")
    if item_kind == "struct":
        mutated_items = module.rust_struct_items(mutated_source, item_name)
    else:
        owner = seal_key.rsplit("::", 1)[0]
        mutated_items = tuple(
            candidate
            for candidate in module.rust_items(mutated_source, item_name)
            if candidate.brace_context
            == (("impl", owner),)
        )
    assert len(mutated_items) == 1
    getattr(module, seal_group)[seal_key] = module._rust_item_token_sha256(
        mutated_items[0]
    )

    errors = module._exact_serve_runtime_episode_production_source_fidelity_errors(
        tmp_path
    )
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
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
            "            runtime_lifecycle_ordinal,\n"
            "        });\n",
            "            runtime_lifecycle_ordinal: None,\n"
            "        });\n",
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
            "    );\n",
            "        None,\n"
            "    );\n",
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
            "    );\n",
            "        None,\n"
            "    );\n",
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
        seal_group = module._EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256
    else:
        mutated_items = tuple(
            candidate
            for candidate in module.rust_items(mutated_source, item_name)
            if candidate.brace_context == context
        )
        seal_group = module._EXACT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256
    assert len(mutated_items) == 1
    seal_group[seal_key] = module._rust_item_token_sha256(mutated_items[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
    module._EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256[seal_key] = (
        module._rust_item_token_sha256(mutated_items[0])
    )

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
    module._EXACT_SERVE_RUNTIME_EPISODE_STRUCT_SHA256[
        "CertifiedServeProducerEpisode"
    ] = module._rust_item_token_sha256(items[0])

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
    module._EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256[
        "CertifiedServeProducerEpisode::drop"
    ] = module._rust_item_token_sha256(mutated_items[0])

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
    module._EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256[
        "older_lifecycle_predates_exact_serve"
    ] = module._rust_item_token_sha256(items[0])
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )
    assert any(
        "duplicate executor boolean projection must remain absent" in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    (
        "relative",
        "seal_group",
        "seal_key",
        "item_name",
        "context",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RESTORE_ITEM_SHA256",
            "restore_certified_serve_tombstones",
            "restore_certified_serve_tombstones",
            (),
            "            last_predecessor_episode_witness: None,\n",
            "            last_predecessor_episode_witness: Some(\n"
            "                ExactServePredecessorEpisodeWitness::for_test(3, 1, 1),\n"
            "            ),\n",
            "restart-restored reservations must begin Ready with no live carrier and no synthetic consumed predecessor witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "advance_executor_once_before_exact_serve",
            "advance_executor_once_before_exact_serve",
            (),
            "    let _ = executor.step(Instant::now(), services)?;\n",
            "    let _ = executor.step_pacemaker_once(Instant::now(), services)?;\n",
            "one exact-Serve turn must execute at most one serialized transition",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "publish_external_lifecycle_owners",
            "publish_external_lifecycle_owners",
            (("impl", "<", "R", ":", "EffectRuntime", ">", "V2EffectExecutor", "<", "R", ">"),),
            "            .set_external_lifecycle_owners(owners)\n",
            "            .set_external_lifecycle_owners({ drop(owners); Vec::new() })\n",
            "exact-Serve owner publication must snapshot every executor-retained owner and preserve the runtime error boundary",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "        self.publish_external_lifecycle_owners()?;\n",
            "        let _ = self.external_lifecycle_owners()?;\n",
            "primary executor witness publisher must fail closed, publish every external owner first",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "            .exact_serve_predecessor_episode_witness(\n"
            "                now,\n"
            "                serve_lifecycle_ordinal,\n"
            "                completion_evidence,\n"
            "            )\n",
            "            .exact_serve_predecessor_episode_witness(\n"
            "                now,\n"
            "                serve_lifecycle_ordinal.saturating_add(1),\n"
            "                completion_evidence,\n"
            "            )\n",
            "primary executor witness publisher must fail closed, publish every external owner first, forward exact completion evidence",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "                completion_evidence,\n"
            "            )\n",
            "                None,\n"
            "            )\n",
            "primary executor witness publisher must fail closed, publish every external owner first, forward exact completion evidence",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "            .map_err(EffectExecutorError::Runtime)\n",
            "            .map_err(EffectExecutorError::Contract)\n",
            "primary executor witness publisher must fail closed, publish every external owner first",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_ITEM_SHA256",
            "older_runtime_lifecycle_predates_retained_response",
            "older_runtime_lifecycle_predates_retained_response",
            (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
            "            .older_lifecycle_predates_retained_response(now, target_lifecycle_ordinal)\n",
            "            .older_lifecycle_predates_exact_serve(now, target_lifecycle_ordinal)\n",
            "retained-response probe must publish complete external ownership and delegate only to its isolated runtime state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "        let io_ordinal = io_ordinal.filter(|ordinal| *ordinal < serve_lifecycle_ordinal);\n",
            "        let io_ordinal = io_ordinal.filter(|ordinal| *ordinal <= serve_lifecycle_ordinal);\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "            .filter(|owned| runtime_capacity_available || !owned.requires_runtime_capacity)\n",
            "            .filter(|_| true)\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "        if runtime_capacity_available {\n"
            "            for completion in &self.local_completions {\n",
            "        if false && runtime_capacity_available {\n"
            "            for completion in &self.local_completions {\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "                        Some(local_ordinal.map_or(ordinal, |current: u128| current.min(ordinal)));\n",
            "                        Some(local_ordinal.map_or(ordinal, |current: u128| current.max(ordinal)));\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "            (Some(io), Some(local)) => Some(io.min(local)),\n",
            "            (Some(io), Some(local)) => Some(io.max(local)),\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256",
            "ProductionV2Services::certified_serve_predecessor_completion_evidence",
            "certified_serve_predecessor_completion_evidence",
            (("impl", "ProductionV2Services"),),
            "    pub(crate) fn certified_serve_predecessor_completion_evidence(\n"
            "        &self,\n",
            "    pub(crate) fn certified_serve_predecessor_completion_evidence(\n"
            "        &mut self,\n",
            "completion projection must be non-consuming, capacity-gated, strictly older, least-ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            retained_response_predecessor_retry_attempted: false,\n",
            "            retained_response_predecessor_retry_attempted: true,\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            exact_serve_target_ordinal: None,\n",
            "            exact_serve_target_ordinal: Some(1),\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            exact_serve_predecessor_retry_attempted: false,\n",
            "            exact_serve_predecessor_retry_attempted: true,\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            retained_response_predecessor_target_ordinal: None,\n",
            "            retained_response_predecessor_target_ordinal: Some(1),\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            exact_serve_predecessor_physically_present: false,\n",
            "            exact_serve_predecessor_physically_present: true,\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            exact_serve_predecessor_episode: 0,\n",
            "            exact_serve_predecessor_episode: 1,\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "with_driver_and_lifecycle_ordinals",
            "with_driver_and_lifecycle_ordinals",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            exact_serve_predecessor_witness: None,\n",
            "            exact_serve_predecessor_witness:\n"
            "                ExactServePredecessorEpisodeWitness::try_new(2, 1, 1),\n",
            "runtime construction must initialize the isolated retained-response probe and selected-Serve predecessor episode without synthetic state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_active_lifecycle_ordinal",
            "minimum_active_lifecycle_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        self.minimum_active_lifecycle_ordinal_excluding(&[])\n",
            "        self.minimum_active_lifecycle_ordinal_excluding(&self.external_lifecycle_owners)\n",
            "exact-Serve runtime minimum must exclude no active owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_active_lifecycle_ordinal_excluding",
            "minimum_active_lifecycle_ordinal_excluding",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        let _ = self.ingress.oldest_active_lifecycle_ordinal()?;\n",
            "        let _ = self.ingress.oldest_lifecycle_ordinal()?;\n",
            "exact-Serve runtime minimum must deeply validate every FIFO and latent Local FIFO owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_runnable_lifecycle_ordinal",
            "minimum_runnable_lifecycle_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        let _ = self.minimum_active_lifecycle_ordinal()?;\n"
            "        let mut minimum = self.ingress.oldest_lifecycle_ordinal()?;\n",
            "        let mut minimum = self.minimum_active_lifecycle_ordinal()?;\n",
            "exact-Serve predecessor selection must deeply validate all owners before projecting runnable FIFO work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_runnable_lifecycle_ordinal",
            "minimum_runnable_lifecycle_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            if !evidence.validate_exact()\n",
            "            if false && !evidence.validate_exact()\n",
            "completion evidence must validate its integrity and shared-source mint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_runnable_lifecycle_ordinal",
            "minimum_runnable_lifecycle_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "                    .recognizes_minted(evidence.lifecycle_ordinal())\n",
            "                    .recognizes_minted(1)\n",
            "completion evidence must validate its integrity and shared-source mint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "minimum_runnable_lifecycle_ordinal",
            "minimum_runnable_lifecycle_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "                Some(minimum.map_or(lifecycle_ordinal, |ordinal| ordinal.min(lifecycle_ordinal)));\n",
            "                Some(minimum.map_or(lifecycle_ordinal, |ordinal| ordinal.max(lifecycle_ordinal)));\n",
            "completion evidence must validate its integrity and shared-source mint",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "active_lifecycle_uses_ordinal",
            "active_lifecycle_uses_ordinal",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        if self.ingress.uses_lifecycle_ordinal(lifecycle_ordinal)? {\n"
            "            return Ok(true);\n"
            "        }\n",
            "        if self.ingress.uses_lifecycle_ordinal(lifecycle_ordinal)? && false {\n"
            "            return Ok(true);\n"
            "        }\n",
            "exact-Serve collision checks must include bounded-ingress and dormant owners",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "older_lifecycle_predates_exact_serve",
            "older_lifecycle_predates_exact_serve",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            .map(|witness| witness.is_some())\n",
            "            .map(|witness| witness.is_none())\n",
            "selected-Serve boolean projection must delegate exclusively to the witnessed producer seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        if !self.exact_serve_predecessor_physically_present {\n",
            "        if self.exact_serve_predecessor_physically_present {\n",
            "only an observed absence-to-presence transition may checked-increment the producer episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        if self.exact_serve_predecessor_retry_attempted {\n",
            "        if false && self.exact_serve_predecessor_retry_attempted {\n",
            "retry-unadmitted suppression must retain physical presence and cannot mint another witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "            !evidence.validate_exact() || evidence.lifecycle_ordinal() >= serve_lifecycle_ordinal\n",
            "            !evidence.validate_exact() || evidence.lifecycle_ordinal() > serve_lifecycle_ordinal\n",
            "completion evidence must be exact and strictly older than its immutable Serve target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "exact_serve_predecessor_episode_witness",
            "exact_serve_predecessor_episode_witness",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        let minimum = match self.minimum_runnable_lifecycle_ordinal(now, completion_evidence) {\n",
            "        let minimum = match self.minimum_runnable_lifecycle_ordinal(now, None) {\n",
            "exact-Serve comparison must use only owners runnable by one serialized turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "older_lifecycle_predates_retained_response",
            "older_lifecycle_predates_retained_response",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "        if self.retained_response_predecessor_target_ordinal != Some(serve_lifecycle_ordinal) {\n",
            "        if self.exact_serve_target_ordinal != Some(serve_lifecycle_ordinal) {\n",
            "retained-response predecessor probe must not mutate or read the other target's episode state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "step",
            "step",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "                    if self\n"
            "                        .retained_response_predecessor_target_ordinal\n"
            "                        .is_some_and(|target| owner.lifecycle_ordinal() < target)\n"
            "                    {\n"
            "                        self.retained_response_predecessor_retry_attempted = true;\n"
            "                    }\n",
            "",
            "must latch each independently active exact target whose ordinal it predates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_ITEM_SHA256",
            "step",
            "step",
            (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
            "                    if self\n"
            "                        .exact_serve_target_ordinal\n"
            "                        .is_some_and(|target| owner.lifecycle_ordinal() < target)\n"
            "                    {\n"
            "                        self.exact_serve_predecessor_retry_attempted = true;\n"
            "                    }\n",
            "",
            "must latch each independently active exact target whose ordinal it predates",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            """                if let Some(witness) = executor.exact_serve_predecessor_episode_witness(
                    Instant::now(),
                    serve_barrier.scheduler_ordinal(),
                    completion_evidence,
                )? {
                    // A passive Fetch is intentionally absent from the
                    // runnable-owner set. A completed strict predecessor is
                    // projected without consuming it; its exact local ordinal
                    // lets the runtime issue one newer episode witness before
                    // the worker claims capacity and admits the completion.
                    let _ = services
                        .observe_certified_serve_predecessor_episode_witness(serve_barrier, witness)
                        .map_err(V2RunnerError::Service)?;
                }
                let claimed_older_runtime_episode = services
                    .claim_certified_serve_runtime_episode(serve_barrier)
                    .map_err(V2RunnerError::Service)?;
""",
            """                let claimed_older_runtime_episode = services
                    .claim_certified_serve_runtime_episode(serve_barrier)
                    .map_err(V2RunnerError::Service)?;
                if let Some(witness) = executor.exact_serve_predecessor_episode_witness(
                    Instant::now(),
                    serve_barrier.scheduler_ordinal(),
                    completion_evidence,
                )? {
                    // A passive Fetch is intentionally absent from the
                    // runnable-owner set. A completed strict predecessor is
                    // projected without consuming it; its exact local ordinal
                    // lets the runtime issue one newer episode witness before
                    // the worker claims capacity and admits the completion.
                    let _ = services
                        .observe_certified_serve_predecessor_episode_witness(serve_barrier, witness)
                        .map_err(V2RunnerError::Service)?;
                }
""",
            "publish and consume a late predecessor witness before attempting to claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                let mut older_predecessor_remains = false;\n",
            "                let mut older_predecessor_remains = false;\n"
            "                let _ = super::v2_runtime::ExactServePredecessorCompletionEvidence::try_new(\n"
            "                    serve_barrier.scheduler_ordinal(),\n"
            "                );\n",
            "completion evidence must be minted exactly once and only by the reviewed non-consuming ProductionV2Services projection",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                let completion_evidence = services\n"
            "                    .certified_serve_predecessor_completion_evidence(\n"
            "                        executor.remaining_completion_capacity() != 0,\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                    )\n"
            "                    .map_err(V2RunnerError::Service)?;\n"
            "                if let Some(witness) = executor.exact_serve_predecessor_episode_witness(\n",
            "                let completion_evidence = services\n"
            "                    .certified_serve_predecessor_completion_evidence(\n"
            "                        executor.remaining_completion_capacity() == 0,\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                    )\n"
            "                    .map_err(V2RunnerError::Service)?;\n"
            "                if let Some(witness) = executor.exact_serve_predecessor_episode_witness(\n",
            "runner must freshly project exact completion ownership before each of its three selected-Serve witness observations",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                    let completion_evidence = services\n"
            "                        .certified_serve_predecessor_completion_evidence(\n"
            "                            executor.remaining_completion_capacity() != 0,\n"
            "                            serve_barrier.scheduler_ordinal(),\n"
            "                        )\n"
            "                        .map_err(V2RunnerError::Service)?;\n"
            "                    let predecessor_witness = executor.exact_serve_predecessor_episode_witness(\n"
            "                        Instant::now(),\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                        completion_evidence,\n"
            "                    )?;\n"
            "                    if let Some(witness) = predecessor_witness {\n"
            "                        let _ = services\n"
            "                            .observe_certified_serve_predecessor_episode_witness(\n"
            "                                serve_barrier,\n"
            "                                witness,\n"
            "                            )\n"
            "                            .map_err(V2RunnerError::Service)?;\n"
            "                    }\n"
            "                    if predecessor_witness.is_some()\n",
            "                    let completion_evidence = None;\n"
            "                    let predecessor_witness = executor.exact_serve_predecessor_episode_witness(\n"
            "                        Instant::now(),\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                        completion_evidence,\n"
            "                    )?;\n"
            "                    if let Some(witness) = predecessor_witness {\n"
            "                        let _ = services\n"
            "                            .observe_certified_serve_predecessor_episode_witness(\n"
            "                                serve_barrier,\n"
            "                                witness,\n"
            "                            )\n"
            "                            .map_err(V2RunnerError::Service)?;\n"
            "                    }\n"
            "                    if predecessor_witness.is_some()\n",
            "runner must freshly project exact completion ownership before each of its three selected-Serve witness observations",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                let mut older_predecessor_remains = false;\n",
            "                let mut older_predecessor_remains = false;\n"
            "                services.drain_exact_serve_runtime_predecessor(\n"
            "                    &mut executor,\n"
            "                    serve_barrier.scheduler_ordinal(),\n"
            "                )?;\n",
            "runner must drain exactly one strict completion only inside the successfully claimed selected-Serve predecessor episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                    services.drain_exact_serve_runtime_predecessor(\n"
            "                        &mut executor,\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                    )?;\n",
            "                    let _ = serve_barrier;\n",
            "runner must drain exactly one strict completion only inside the successfully claimed selected-Serve predecessor episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                    if predecessor_witness.is_some()\n"
            "                        && services\n",
            "                    if predecessor_witness.is_none()\n"
            "                        && services\n",
            "serialized predecessor step must consume the stable witness and require both that witness and physical capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "                    older_predecessor_remains = predecessor_witness.is_some();\n",
            "                    older_predecessor_remains = predecessor_witness.is_none();\n",
            "every claimed turn must re-publish, consume, and recheck the full witnessed owner set before settlement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNNER_ITEM_SHA256",
            "run_inner",
            "run_inner",
            (),
            "            else {\n"
            "                // Exact admission won the queue-locked race after the\n"
            "                // observation above. Restart at the dedicated target turn.\n"
            "                let _ = wake_rx.recv_timeout(IDLE_POLL);\n"
            "                continue;\n"
            "            };\n",
            "            else {\n"
            "                // Exact admission won the queue-locked race after the\n"
            "                // observation above. Restart at the dedicated target turn.\n"
            "                let _ = wake_rx.recv();\n"
            "                continue;\n"
            "            };\n",
            "queue-locked handoff to an exact target which won the admission race must retain the finite wake bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_INGRESS_ITEM_SHA256",
            "oldest_active_lifecycle_ordinal",
            "oldest_active_lifecycle_ordinal",
            (("impl", "<", "C", ":", "ExactRuntimeCommandIdentity", ">", "BoundedIngress", "<", "C", ">"),),
            "                return Err(EnqueueError::FailClosed);\n",
            "                continue;\n",
            "latent Local FIFO reservations must retain exact minted identity but remain passive until a runnable occurrence materializes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_INGRESS_ITEM_SHA256",
            "uses_lifecycle_ordinal",
            "uses_lifecycle_ordinal",
            (("impl", "<", "C", ":", "ExactRuntimeCommandIdentity", ">", "BoundedIngress", "<", "C", ">"),),
            "        if self\n"
            "            .dormant_local_fifo_reservations\n"
            "            .iter()\n"
            "            .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)\n"
            "        {\n"
            "            return Ok(true);\n"
            "        }\n",
            "        if self\n"
            "            .dormant_local_fifo_reservations\n"
            "            .iter()\n"
            "            .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)\n"
            "        {\n"
            "            return Ok(false);\n"
            "        }\n",
            "latent Local FIFO reservations must collide with reused exact-Serve ordinals",
        ),
    ),
)
def test_exact_serve_cross_file_boundaries_survive_item_digest_refresh(
    tmp_path: Path,
    relative: str,
    seal_group: str,
    seal_key: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Every cross-file exact-Serve seam remains semantic after resealing."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    items = tuple(
        item
        for item in module.rust_items(source, item_name)
        if item.brace_context == context
    )
    assert len(items) == 1, (item_name, [item.brace_context for item in items])
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
    rebound_digest = module._rust_item_token_sha256(mutated_items[0])
    getattr(module, seal_group)[seal_key] = rebound_digest
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)
    if relative == "crates/iroha_core/src/sumeragi/v2_runtime.rs":
        if item_name == "with_driver_and_lifecycle_ordinals":
            module._SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256[
                "with_driver_and_lifecycle_ordinals"
            ] = rebound_digest
        elif item_name == "step":
            module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256["runtime_step"] = (
                rebound_digest
            )
            module._SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256["step"] = (
                rebound_digest
            )
    if relative == "crates/iroha_core/src/sumeragi/v2_runner.rs" and item_name == "run_inner":
        for alias_group, alias_key in (
            ("_PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256", "run_inner"),
            ("_PRODUCTION_LOCAL_RUNNER_SERVICE_ITEM_SHA256", "run_inner"),
            ("_PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256", "run_inner"),
            ("_TIMEOUT_VOTE_EPISODE_RUST_ITEM_SHA256", "runner::run_inner"),
            (
                "_PRODUCTION_RETAINED_RESPONSE_ESCAPE_LATCH_RUST_ITEM_SHA256",
                "runner::run_inner",
            ),
            ("_SERVICED_CANDIDATE_V4_RUNNER_ITEM_SHA256", "run_inner"),
            ("_LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256", "run_inner"),
        ):
            getattr(module, alias_group)[alias_key] = rebound_digest

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors

@pytest.mark.parametrize(
    (
        "relative",
        "seal_group",
        "item_name",
        "context",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        for fresh_ticket_ordinal in first_ticket_ordinal..=later_ordinal {\n",
            "        for fresh_ticket_ordinal in first_ticket_ordinal..later_ordinal {\n",
            "strict exact-Serve regression must exclude an equal-or-later completion through every non-newer ticket",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "            Some(older_task.lifecycle_ordinal()),\n"
            "            \"the least strict local predecessor must reopen the exact Serve episode\"\n",
            "            None,\n"
            "            \"the least strict local predecessor must reopen the exact Serve episode\"\n",
            "completion-evidence regression must project the least strict local predecessor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "                .certified_serve_predecessor_completion_evidence(false, first_ticket_ordinal,)\n",
            "                .certified_serve_predecessor_completion_evidence(true, first_ticket_ordinal,)\n",
            "completion requiring runtime capacity must not reopen Serve while capacity is unavailable",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "exact_serve_predecessor_episode_services_older_local_without_admitting_later_io",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "            Some(later_ordinal),\n"
            "            \"the exact I/O completion becomes eligible only below a later ticket\"\n",
            "            None,\n"
            "            \"the exact I/O completion becomes eligible only below a later ticket\"\n",
            "I/O completion must become evidence only below a strictly later ticket",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "repeated_exact_serve_claims_close_all_older_sources_before_later_io",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "            .finish_certified_serve_runtime_episode_turn(barrier, false)\n",
            "            .finish_certified_serve_runtime_episode_turn(barrier, true)\n",
            "repeated exact-Serve claims must stay sealed after the complete older-owner set is exhausted unless a newer witness arrives",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        assert!(\n"
            "            !command_tx\n"
            "                .serve_runtime_predecessor_capacity_available(barrier)\n"
            "                .expect(\"inspect the full frozen prefix\"),\n",
            "        assert!(\n"
            "            command_tx\n"
            "                .serve_runtime_predecessor_capacity_available(barrier)\n"
            "                .expect(\"inspect the full frozen prefix\"),\n",
            "full Control prefix must deny predecessor capacity until its sole physical slot drains",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "worker_completion_is_retained_behind_a_full_runtime_fifo",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "                io.record_completion_service_attempt(0),\n",
            "                !io.record_completion_service_attempt(0),\n",
            "full runtime FIFO must retain the oldest worker completion and accrue bounded service debt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "production_drain_publishes_worker_completion_behind_full_runtime_fifo",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        service.retire_held_io_completion();\n",
            "        let _ = service.held_io_completion.take();\n",
            "production completion drain must explicitly acknowledge the exact held worker result",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "drained_exact_retransmission_gets_fresh_scheduler_ordinal",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        assert!(retry_barrier.scheduler_ordinal() > first_barrier.scheduler_ordinal());\n",
            "        assert!(retry_barrier.scheduler_ordinal() >= first_barrier.scheduler_ordinal());\n",
            "drained exact retransmission must receive a fresh scheduler ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/tests/v2_worker_certified_serve_budget_cases.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "certified_serve_future_slot_blocks_control_and_consensus_replenishment",
            (),
            "            for class in [V2IoAdmissionClass::Consensus, V2IoAdmissionClass::Control] {\n",
            "            for class in [V2IoAdmissionClass::Control] {\n",
            "reserved future Serve slot must block both later Consensus and Control replenishment",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        let replenished =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 2);\n",
            "        let replenished =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 3);\n",
            "witness regression must model the exact next producer episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        let first =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 1, 1);\n",
            "        let first =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 1);\n",
            "witness regression must begin with exact predecessor one at episode one",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        let conflicting =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 1);\n",
            "        let conflicting =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 2);\n",
            "witness regression must model a same-episode exact-evidence conflict",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        let skipped =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 3);\n",
            "        let skipped =\n"
            "            ExactServePredecessorEpisodeWitness::for_test(barrier.scheduler_ordinal(), 2, 2);\n",
            "witness regression must model a skipped producer episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "            !command_tx\n"
            "                .observe_serve_predecessor_episode_witness(barrier, first)\n"
            "                .expect(\"same physical episode must coalesce\")\n",
            "            command_tx\n"
            "                .observe_serve_predecessor_episode_witness(barrier, first)\n"
            "                .expect(\"same physical episode must coalesce\")\n",
            "witness regression must prove that Complete remains sealed for an identical episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "                .observe_serve_predecessor_episode_witness(barrier, conflicting)\n"
            "                .is_err(),\n",
            "                .observe_serve_predecessor_episode_witness(barrier, conflicting)\n"
            "                .is_ok(),\n",
            "witness regression must reject conflicting evidence within one episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "                .observe_serve_predecessor_episode_witness(barrier, skipped)\n"
            "                .is_err(),\n",
            "                .observe_serve_predecessor_episode_witness(barrier, skipped)\n"
            "                .is_ok(),\n",
            "witness regression must reject a skipped consumer episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "            !command_tx\n"
            "                .observe_serve_predecessor_episode_witness(barrier, replenished)\n"
            "                .expect(\"repeated replenishment witness must stutter\")\n",
            "            command_tx\n"
            "                .observe_serve_predecessor_episode_witness(barrier, replenished)\n"
            "                .expect(\"repeated replenishment witness must stutter\")\n",
            "exactly the next witness must reopen Complete once and then stutter",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256",
            "completed_exact_serve_episode_reopens_once_for_new_runtime_witness",
            (("#", "[", "cfg", "(", "test", ")", "]", "pub", "(", "super", ")", "mod", "tests"),),
            "        assert!(matches!(committed, CertifiedServeCommit::Queued));\n",
            "        assert!(matches!(committed, CertifiedServeCommit::Coalesced));\n",
            "the reopened owner must retire through real target delivery and the finite producer handoff",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "                .is_none(),\n"
            "            \"passive Fetch transport work alone cannot block Serve\"\n",
            "                .is_some(),\n"
            "            \"passive Fetch transport work alone cannot block Serve\"\n",
            "passive Fetch alone must not mint or block on a predecessor witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        let fetch_ordinal = fixture\n"
            "            .lifecycle_ordinals\n"
            "            .reserve_one()\n"
            "            .expect(\"reserve the passive Fetch lifecycle before Serve\");\n",
            "        let fetch_ordinal = 2;\n",
            "the concrete late-runnable regression must reserve passive Fetch ownership before the Serve target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert_eq!(witness.episode(), 1);\n",
            "        assert_eq!(witness.episode(), 2);\n",
            "late BodyAvailable materialization must mint episode one for the original earlier Fetch owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "            Some(witness),\n"
            "            \"one continuous predecessor prefix retains one witness across target probes\"\n",
            "            None,\n"
            "            \"one continuous predecessor prefix retains one witness across target probes\"\n",
            "one continuous late predecessor prefix must retain the identical witness across the alternate target probe",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "                .step(Instant::now(), &mut services)\n"
            "                .expect(\"the reopened predecessor owns the next serialized step\"),\n",
            "                .step_pacemaker_once(Instant::now(), &mut services)\n"
            "                .expect(\"the reopened predecessor owns the next serialized step\"),\n",
            "the reopened predecessor must consume one real runtime completion in one serialized step",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "            services.store_tasks.len(),\n            1,\n",
            "            services.store_tasks.len(),\n            0,\n",
            "the reopened BodyAvailable transition must produce exactly one Store successor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "            services.store_tasks[0].lifecycle_ordinal(),\n"
            "            fetch_ordinal,\n",
            "            services.store_tasks[0].lifecycle_ordinal(),\n"
            "            serve_ordinal,\n",
            "the Store successor must retain the immutable original Fetch owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "                .expect(\"an incomplete Store remains passive\")\n"
            "                .is_none(),\n",
            "                .expect(\"an incomplete Store remains passive\")\n"
            "                .is_some(),\n",
            "an incomplete asynchronous Store must remain passive and cannot veto Serve",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert_eq!(replenished.episode(), 2);\n",
            "        assert_eq!(replenished.episode(), 1);\n",
            "tracked completed Store must retain its immutable owner and open exactly the next finite predecessor episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_EFFECT_REGRESSION_TEST_SHA256",
            "late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "                .older_runtime_lifecycle_predates_retained_response(\n"
            "                    Instant::now(),\n"
            "                    retained_response_ordinal,\n"
            "                )\n"
            "                .expect(\"exercise the published retained-response predecessor probe\")\n",
            "                .exact_serve_predecessor_episode_witness(\n"
            "                    Instant::now(),\n"
            "                    retained_response_ordinal,\n"
            "                )\n"
            "                .expect(\"exercise the published retained-response predecessor probe\")\n"
            "                .is_some()\n",
            "the concrete late-runnable regression must execute the published isolated retained-response wrapper",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "                .older_lifecycle_predates_retained_response(start, retained_response_ordinal)\n"
            "                .expect(\"alternate retained-response target sees the same older owner\")\n",
            "                .older_lifecycle_predates_exact_serve(start, retained_response_ordinal)\n"
            "                .expect(\"alternate retained-response target sees the same older owner\")\n",
            "alternating-target regression must exercise the isolated retained-response probe",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "            Some(first_witness),\n"
            "            \"selected Serve retains one monotone witness across the legacy target probe\"\n",
            "            None,\n"
            "            \"selected Serve retains one monotone witness across the legacy target probe\"\n",
            "selected-Serve witness must remain stable after an alternate target probe",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert!(runtime.exact_serve_predecessor_retry_attempted);\n",
            "        assert!(!runtime.exact_serve_predecessor_retry_attempted);\n",
            "one retry-unadmitted step must latch both independently active exact targets",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        assert!(!runtime.retained_response_predecessor_retry_attempted);\n",
            "        assert!(runtime.retained_response_predecessor_retry_attempted);\n",
            "settling the shared older owner must clear both retry latches without witness regression",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "        let completed_witness = runtime\n"
            "            .exact_serve_predecessor_episode_witness(\n"
            "                start,\n"
            "                completed_target,\n"
            "                Some(completed_evidence),\n"
            "            )\n",
            "        let completed_witness = runtime\n"
            "            .exact_serve_predecessor_episode_witness(\n"
            "                start,\n"
            "                completed_target,\n"
            "                None,\n"
            "            )\n",
            "only exact completion evidence may turn a passive service owner into one finite runnable predecessor episode",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "_EXACT_SERVE_RUNTIME_EPISODE_RUNTIME_REGRESSION_TEST_SHA256",
            "restart_dormant_local_fifo_reservation_survives_full_class_churn",
            (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
            "            Err(EnqueueError::FailClosed),\n"
            "            \"ReuseDormant after latent-slot removal cannot recreate the drained stage\"\n",
            "            Ok(()),\n"
            "            \"ReuseDormant after latent-slot removal cannot recreate the drained stage\"\n",
            "drained restart-dormant lifecycle must not resurrect after its latent slot is consumed",
        ),
    ),
)
def test_exact_serve_runtime_episode_regressions_survive_item_digest_refresh(
    tmp_path: Path,
    relative: str,
    seal_group: str,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Every exact-Serve regression retains behavior after its own reseal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    items = tuple(
        item
        for item in module.rust_items(source, item_name)
        if item.brace_context == context
    )
    assert len(items) == 1, (item_name, [item.brace_context for item in items])
    item = items[0]
    assert item.source.count(old) == 1, (item_name, old)
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
    getattr(module, seal_group)[item_name] = module._rust_item_token_sha256(
        mutated_items[0]
    )
    rebind_changed_same_round_expanded_source_seal(module, tmp_path)

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(
        expected_error in error and "exact reviewed token digest" not in error
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
    module._EXACT_SERVE_RUNTIME_EPISODE_WORKER_ITEM_SHA256[seal_key] = (
        module._rust_item_token_sha256(mutated_items[0])
    )

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    name = (
        "final_serve_retirement_yields_one_producer_episode_before_replenishment"
    )
    context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "test",
            ")",
            "]",
            "pub",
            "(",
            "super",
            ")",
            "mod",
            "tests",
        ),
    )
    items = [
        item
        for item in module.rust_items(source, name)
        if item.brace_context == context
    ]
    assert len(items) == 1
    item = items[0]
    old = "            Err(CertifiedServeIngressReserveError::Busy)\n"
    new = "            Err(CertifiedServeIngressReserveError::Rejected)\n"
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
    module._EXACT_SERVE_RUNTIME_EPISODE_REGRESSION_TEST_SHA256[name] = (
        module._rust_item_token_sha256(mutated_item)
    )

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
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
            "    if !recovering_interrupted_tip && older_runtime_episode_claimed {\n",
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
            "        || service(CertifiedServeBarrierLivenessAction::Pacemaker),\n",
            "        || service(CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix),\n",
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

@pytest.mark.parametrize(
    (
        "seal_key",
        "source_kind",
        "item_kind",
        "item_name",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "runner::CertifiedServeBarrierLivenessAction",
            "runner",
            "enum",
            "CertifiedServeBarrierLivenessAction",
            "    Pacemaker,\n",
            "    PacemakerBypass,\n",
            "selected-Serve liveness action vocabulary must remain closed",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "                        recovery.service_timeout_vote_episode()\n",
            "                        recovery.service_pacemaker()\n",
            "selected-Serve regression must drive the real ingress, worker, runtime, TC, and EnterView terminal",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "    for older_runtime_episode_claimed in [true, false] {\n",
            "    for older_runtime_episode_claimed in [true, true] {\n",
            "completed selected-Serve predecessor claims must retain the same bounded pacemaker turn",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "    .expect_err(\"live runner propagates a typed pacemaker failure\");\n",
            "    .expect(\"live runner swallows a typed pacemaker failure\");\n",
            "selected-Serve pacemaker regression must propagate typed failure and suppress only interrupted-tip recovery",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "            let older_runtime_episode_claimed = recovery\n"
            "                .service_exact_serve_runtime_prefix()\n"
            "                .expect(\"service the exact selected-Serve runtime prefix\");\n",
            "            let older_runtime_episode_claimed = false;\n",
            "selected-Serve regression must drive the real ingress, worker, runtime, TC, and EnterView terminal",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "                older_runtime_episode_claimed,\n"
            "                |action| match action {\n",
            "                false,\n"
            "                |action| match action {\n",
            "selected-Serve regression must drive the real ingress, worker, runtime, TC, and EnterView terminal",
        ),
        (
            "runner::complete_certified_serve_episode_cannot_veto_pacemaker",
            "runner_test",
            "item",
            "complete_certified_serve_episode_cannot_veto_pacemaker",
            "        let mut late_passive_fetch =\n"
            "            super::super::v2_worker::tests::SelectedServeTimeoutRecoveryFixture::new_late_passive_fetch();\n"
            "        late_passive_fetch.assert_late_passive_fetch_completion_reopens_selected_serve();\n",
            "",
            "selected-Serve regression must execute the real late-passive-Fetch completion, target release, and producer handoff",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryMode",
            "worker",
            "enum",
            "SelectedServeTimeoutRecoveryMode",
            "        LatePassiveFetch,\n",
            "        LatePassiveFetchBypass,\n",
            "selected-Serve fixture mode vocabulary must remain closed",
        ),
        (
            "worker::SelectedServeLatePassiveFetch",
            "worker",
            "struct",
            "SelectedServeLatePassiveFetch",
            "        body_store: V2BodyStore,\n",
            "",
            "late-passive-Fetch fixture must retain the exact body store, immutable task owner, manifest, and body",
        ),
        (
            "worker::SelectedServeLatePassiveFetch",
            "worker",
            "struct",
            "SelectedServeLatePassiveFetch",
            "        task: BodyFetchTask,\n",
            "",
            "late-passive-Fetch fixture must retain the exact body store, immutable task owner, manifest, and body",
        ),
        (
            "worker::SelectedServeLatePassiveFetch",
            "worker",
            "struct",
            "SelectedServeLatePassiveFetch",
            "        manifest: wire::PayloadManifest,\n",
            "",
            "late-passive-Fetch fixture must retain the exact body store, immutable task owner, manifest, and body",
        ),
        (
            "worker::SelectedServeLatePassiveFetch",
            "worker",
            "struct",
            "SelectedServeLatePassiveFetch",
            "        body: Vec<u8>,\n",
            "",
            "late-passive-Fetch fixture must retain the exact body store, immutable task owner, manifest, and body",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture",
            "worker",
            "struct",
            "SelectedServeTimeoutRecoveryFixture",
            "        late_passive_fetch: Option<SelectedServeLatePassiveFetch>,\n",
            "",
            "selected-Serve regression must retain every real ingress, runtime, worker, and observation owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture",
            "worker",
            "struct",
            "SelectedServeTimeoutRecoveryFixture",
            "        missing_proposal_request: AuthenticatedCertifiedBodyRequest,\n",
            "",
            "selected-Serve regression must retain every real ingress, runtime, worker, and observation owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture",
            "worker",
            "struct",
            "SelectedServeTimeoutRecoveryFixture",
            "        missing_proposal_request_hash: HashOf<wire::CertifiedBodyRequest>,\n",
            "",
            "selected-Serve regression must retain every real ingress, runtime, worker, and observation owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new",
            "worker",
            "method",
            "new",
            "            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)\n",
            "            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)\n",
            "the timeout-recovery fixture constructor must select only its exact closed mode",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_late_passive_fetch",
            "worker",
            "method",
            "new_late_passive_fetch",
            "            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::LatePassiveFetch)\n",
            "            Self::new_for_mode(SelectedServeTimeoutRecoveryMode::TimeoutRecovery)\n",
            "the late-passive-Fetch fixture constructor must select only its exact closed mode",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                    .take(2)\n",
            "                    .take(1)\n",
            "selected-Serve fixture must enqueue exactly two distinct remote timeout signers",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                        timeout_owner.lifecycle_ordinal(),\n"
            "                        1,\n"
            "                        \"the height-start timeout owns the first actor-global scheduler position\"\n",
            "                        timeout_owner.lifecycle_ordinal(),\n"
            "                        2,\n"
            "                        \"the height-start timeout owns the first actor-global scheduler position\"\n",
            "selected-Serve fixture must freeze the height-start timeout at actor-global ordinal one",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                allow_fixture_block_payload(&mut services.context);\n",
            "                let _ = &services.context;\n",
            "late-passive-Fetch mode must widen the exact context and rebuild its recovery authority before cloning any context-bound service",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                    services.context.height,\n"
            "                    [0xF4; 32],\n"
            "                    services.active_tag.view(),\n"
            "                    false,\n",
            "                    services.context.height,\n"
            "                    [0xF5; 32],\n"
            "                    services.active_tag.view(),\n"
            "                    false,\n",
            "late-passive-Fetch mode must widen the exact context and rebuild its recovery authority before cloning any context-bound service",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                services.chunk_root = runtime_directory.path().join(\"chunks\");\n",
            "                services.chunk_root = PathBuf::new();\n",
            "late-passive-Fetch mode must retain an isolated chunk root before dispatching body work",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                SelectedServeTimeoutRecoveryMode::LatePassiveFetch => {\n"
            "                    Duration::from_secs(24 * 60 * 60)\n"
            "                }\n",
            "                SelectedServeTimeoutRecoveryMode::LatePassiveFetch => {\n"
            "                    Duration::from_millis(1)\n"
            "                }\n",
            "selected-Serve fixture must keep only timeout recovery due while the late-Fetch pipeline owns one long non-due clock",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                    executor\n"
            "                        .arm_live_clocks(late_dispatch_at)\n"
            "                        .expect(\"arm non-due late-passive-Fetch clocks\");\n",
            "",
            "late-passive-Fetch mode must arm exactly one fresh non-due clock",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                            .step(late_dispatch_at, &mut services)\n"
            "                            .expect(\"dispatch the signed proposal into passive Fetch work\"),\n",
            "                            .step_pacemaker_once(late_dispatch_at, &mut services)\n"
            "                            .expect(\"dispatch the signed proposal into passive Fetch work\"),\n",
            "late-passive-Fetch mode must arm and reuse one non-due instant for a real signed Proposal and one serialized production step establishing exactly one passive Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                            .step(late_dispatch_at, &mut services)\n",
            "                            .step(Instant::now(), &mut services)\n",
            "late-passive-Fetch mode must arm and reuse one non-due instant for a real signed Proposal and one serialized production step establishing exactly one passive Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                        &proposal.signature_preimage(),\n",
            "                        b\"forged late-passive-Fetch proposal\",\n",
            "late-passive-Fetch mode must arm and reuse one non-due instant for a real signed Proposal and one serialized production step establishing exactly one passive Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                        executor.status().pending_fetches,\n"
            "                        1,\n",
            "                        executor.status().pending_fetches,\n"
            "                        0,\n",
            "late-passive-Fetch mode must arm and reuse one non-due instant for a real signed Proposal and one serialized production step establishing exactly one passive Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            ".checked_add(1)",
            ".checked_add(2)",
            "selected Serve must take exactly the next shared actor-global ordinal after the passive Fetch",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "            if mode == SelectedServeTimeoutRecoveryMode::TimeoutRecovery {\n",
            "            if mode != SelectedServeTimeoutRecoveryMode::TimeoutRecovery {\n",
            "only timeout-recovery mode may enqueue the two remote TimeoutVote owners",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                context.roster.len(),\n"
            "                4,\n",
            "                context.roster.len(),\n"
            "                3,\n",
            "selected-Serve fixture must use four validators and a non-leader local timeout owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "            let (command_tx, command_rx, admission) = test_io_command_channel(8);\n",
            "            let (command_tx, command_rx, admission) = test_io_command_channel(7);\n",
            "selected-Serve fixture must share one actor-global ordinal and tracked completion owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "            ingress.require_leader_wire_lifecycle_gate();\n",
            "",
            "selected-Serve fixture must require and bind the real Serve and leader-wire gates",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                    lifecycle_ordinals.clone(),\n",
            "                    RuntimeLifecycleOrdinalSource::after_high_watermark(0),\n",
            "selected-Serve fixture must bind leader-wire admission to the same lifecycle source",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                    missing_request.request(),\n"
            "                    authenticated_via,\n",
            "                    missing_request.request(),\n"
            "                    services.local_peer.clone(),\n",
            "selected-Serve fixture must admit a remote authenticated missing-proposal request",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                        &timeout_vote.signature_preimage(),\n",
            "                        b\"forged selected-Serve timeout vote\",\n",
            "selected-Serve fixture remote timeout votes must be signed and authenticated by their roster sources",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                lifecycle_ordinals,\n"
            "            )\n"
            "            .expect(\"construct selected-Serve serialized runtime\");\n",
            "                RuntimeLifecycleOrdinalSource::after_high_watermark(0),\n"
            "            )\n"
            "            .expect(\"construct selected-Serve serialized runtime\");\n",
            "selected-Serve fixture runtime must consume the shared actor-global lifecycle source",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                        body,\n"
            "                    })\n",
            "                        body: Vec::new(),\n"
            "                    })\n",
            "late-passive-Fetch mode must retain the exact dispatched task, manifest, body, and isolated durable store",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::new_for_mode",
            "worker",
            "method",
            "new_for_mode",
            "                late_passive_fetch,\n"
            "                executor,\n"
            "                services,\n",
            "                late_passive_fetch: None,\n"
            "                executor,\n"
            "                services,\n",
            "selected-Serve fixture must retain the authenticated target and complete late-Fetch state with the live executor and services",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let completion_evidence = self\n"
            "                .services\n"
            "                .certified_serve_predecessor_completion_evidence(\n"
            "                    self.executor.remaining_completion_capacity() != 0,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )?;\n"
            "            if let Some(witness) = self\n"
            "                .executor\n"
            "                .exact_serve_predecessor_episode_witness(\n"
            "                    Instant::now(),\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                    completion_evidence,\n"
            "                )\n"
            "                .map_err(|error| error.to_string())?\n"
            "            {\n"
            "                let _ = self\n"
            "                    .services\n"
            "                    .observe_certified_serve_predecessor_episode_witness(barrier, witness)?;\n"
            "            }\n",
            "",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let completion_evidence = self\n"
            "                .services\n"
            "                .certified_serve_predecessor_completion_evidence(\n"
            "                    self.executor.remaining_completion_capacity() != 0,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )?;\n"
            "            if let Some(witness) = self\n",
            "            let completion_evidence = self\n"
            "                .services\n"
            "                .certified_serve_predecessor_completion_evidence(\n"
            "                    self.executor.remaining_completion_capacity() == 0,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )?;\n"
            "            if let Some(witness) = self\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let _ = self\n"
            "                .services\n"
            "                .drain_exact_serve_runtime_predecessor(\n"
            "                    &mut self.executor,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )\n"
            "                .map_err(|error| error.to_string())?;\n"
            "            let completion_evidence = self\n"
            "                .services\n"
            "                .certified_serve_predecessor_completion_evidence(\n"
            "                    self.executor.remaining_completion_capacity() != 0,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )?;\n",
            "            let _ = self\n"
            "                .services\n"
            "                .drain_exact_serve_runtime_predecessor(\n"
            "                    &mut self.executor,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )\n"
            "                .map_err(|error| error.to_string())?;\n"
            "            let completion_evidence = None;\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "                .ok_or_else(|| \"selected-Serve fixture lost its exact barrier\".to_owned())?;\n"
            "            let completion_evidence = self\n",
            "                .ok_or_else(|| \"selected-Serve fixture lost its exact barrier\".to_owned())?;\n"
            "            let _ = self.services.drain_exact_serve_runtime_predecessor(\n"
            "                &mut self.executor,\n"
            "                barrier.scheduler_ordinal(),\n"
            "            );\n"
            "            let completion_evidence = self\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let claimed = self\n"
            "                .services\n"
            "                .claim_certified_serve_runtime_episode(barrier)?;\n",
            "            let claimed = true;\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let _ = self\n"
            "                .services\n"
            "                .drain_exact_serve_runtime_predecessor(\n"
            "                    &mut self.executor,\n"
            "                    barrier.scheduler_ordinal(),\n"
            "                )\n"
            "                .map_err(|error| error.to_string())?;\n",
            "            let _ = Ok::<(), String>(());\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            if predecessor_witness.is_some()\n"
            "                && self\n"
            "                    .services\n"
            "                    .certified_serve_runtime_predecessor_capacity_available(barrier)?\n",
            "            if predecessor_witness.is_some()\n"
            "                || self\n"
            "                    .services\n"
            "                    .certified_serve_runtime_predecessor_capacity_available(barrier)?\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            ".step(Instant::now(), &mut self.services)",
            ".step_pacemaker_once(Instant::now(), &mut self.services)",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "            let older_predecessor_remains = predecessor_witness.is_some();\n",
            "            let older_predecessor_remains = false;\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_exact_serve_runtime_prefix",
            "worker",
            "method",
            "service_exact_serve_runtime_prefix",
            "                .finish_certified_serve_runtime_episode_turn(barrier, older_predecessor_remains)?;\n",
            "                .finish_certified_serve_runtime_episode_turn(barrier, false)?;\n",
            "selected-Serve exact runtime prefix must observe before claim, drain the strict completion, service at most one witnessed capacity-gated predecessor, then re-observe and finish",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the passive Fetch alone cannot reopen the completed episode\"),\n",
            "                self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the passive Fetch alone cannot reopen the completed episode\"),\n",
            "the integrated late-Fetch regression must first seal a real selected-Serve episode Complete while passive Fetch work remains non-runnable",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            assert!(\n"
            "                self.service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"complete the initially selected Serve predecessor episode\")\n"
            "            );\n"
            "            assert!(\n"
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the passive Fetch alone cannot reopen the completed episode\"),\n"
            "                \"transport-passive Fetch work is not runnable reducer progress\"\n"
            "            );\n"
            "\n"
            "            assert_eq!(\n"
            "                self.executor\n"
            "                    .complete_body_reconstruction(\n"
            "                        &late.task,\n"
            "                        late.manifest.clone(),\n"
            "                        late.body.clone(),\n"
            "                        &mut self.services,\n"
            "                    )\n"
            "                    .expect(\"complete the exact passive body reconstruction\"),\n"
            "                CompletionDisposition::Accepted\n"
            "            );\n"
            "            assert!(\n"
            "                self.service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the late BodyAvailable successor reopens the Serve episode\")\n"
            "            );\n",
            "            assert_eq!(\n"
            "                self.executor\n"
            "                    .complete_body_reconstruction(\n"
            "                        &late.task,\n"
            "                        late.manifest.clone(),\n"
            "                        late.body.clone(),\n"
            "                        &mut self.services,\n"
            "                    )\n"
            "                    .expect(\"complete the exact passive body reconstruction\"),\n"
            "                CompletionDisposition::Accepted\n"
            "            );\n"
            "            assert!(\n"
            "                self.service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the late BodyAvailable successor reopens the Serve episode\")\n"
            "            );\n"
            "\n"
            "            assert!(\n"
            "                self.service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"complete the initially selected Serve predecessor episode\")\n"
            "            );\n"
            "            assert!(\n"
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the passive Fetch alone cannot reopen the completed episode\"),\n"
            "                \"transport-passive Fetch work is not runnable reducer progress\"\n"
            "            );\n",
            "integrated late-Fetch completion must seal Complete before reconstruction, acknowledge Store and Validate in order, retire the owner, drain Serve, and only then hand off producer ownership",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                        late.body.clone(),\n",
            "                        Vec::new(),\n",
            "a real accepted BodyAvailable completion must reopen the previously Complete selected-Serve episode",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                store_task.lifecycle_ordinal(),\n"
            "                fetch_ordinal,\n",
            "                store_task.lifecycle_ordinal(),\n"
            "                fetch_ordinal + 1,\n",
            "the reopened body pipeline must execute Store and publish its tracked completion under the immutable original Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            assert!(\n"
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"an incomplete Store cannot reopen the completed episode\"),\n"
            "                \"active Store work remains passive until its tracked completion exists\"\n"
            "            );\n",
            "",
            "the reopened body pipeline must execute Store and publish its tracked completion under the immutable original Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                V2IoCompletion::Stored(stored),\n"
            "                Some(fetch_ordinal),\n",
            "                V2IoCompletion::Stored(stored),\n"
            "                None,\n",
            "the reopened body pipeline must execute Store and publish its tracked completion under the immutable original Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            self.command_rx.complete_work(store_task.id());\n",
            "",
            "the reopened body pipeline must execute Store and publish its tracked completion under the immutable original Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                validation_task.lifecycle_ordinal(),\n"
            "                fetch_ordinal,\n",
            "                validation_task.lifecycle_ordinal(),\n"
            "                fetch_ordinal + 1,\n",
            "Stored must causally re-fanout exactly one Validate command under the same immutable Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            assert!(\n"
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"an incomplete Validate cannot reopen the completed episode\"),\n"
            "                \"active Validate work remains passive until its tracked completion exists\"\n"
            "            );\n",
            "",
            "Stored must causally re-fanout exactly one Validate command under the same immutable Fetch owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            assert!(matches!(\n"
            "                &validated,\n"
            "                BodyValidationCompletion::Rejected { work_id, reason }\n"
            "                    if *work_id == validation_task.id()\n"
            "                        && reason == \"deterministic late-passive-Fetch rejection\"\n"
            "            ));\n",
            "",
            "Validate must terminate deterministically through an exact tracked rejection completion rather than opening an unbounded Sign suffix",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            self.command_rx.complete_work(validation_task.id());\n",
            "",
            "Validate must terminate deterministically through an exact tracked rejection completion rather than opening an unbounded Sign suffix",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                V2IoCompletion::Validated(validated),\n"
            "                Some(fetch_ordinal),\n",
            "                V2IoCompletion::Validated(validated),\n"
            "                None,\n",
            "Validate must terminate deterministically through an exact tracked rejection completion rather than opening an unbounded Sign suffix",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                !self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the retired body pipeline leaves no older predecessor\"),\n",
            "                self\n"
            "                    .service_exact_serve_runtime_prefix()\n"
            "                    .expect(\"the retired body pipeline leaves no older predecessor\"),\n",
            "the ValidationFailed terminal must drain the original owner and leave the selected-Serve predecessor episode Complete again",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                    && request.request_hash() == self.missing_proposal_request_hash\n",
            "                    && request.request_hash() != self.missing_proposal_request_hash\n",
            "after the older owner retires, the exact selected Serve must commit and materialize only its authenticated retained request",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            let requester = self.missing_proposal_request.request().requester.clone();\n",
            "            let producer_episode = self\n"
            "                .services\n"
            "                .try_begin_certified_serve_producer_episode()\n"
            "                .expect(\"inspect producer ownership after exact Serve drain\")\n"
            "                .expect(\"the exact Serve completion must reopen one producer episode\");\n"
            "            assert!(\n"
            "                self.services\n"
            "                    .try_begin_certified_serve_producer_episode()\n"
            "                    .is_err(),\n"
            "                \"one live producer lease must reject a nested ownership claim\"\n"
            "            );\n"
            "            drop(producer_episode);\n"
            "            let requester = self.missing_proposal_request.request().requester.clone();\n",
            "integrated late-Fetch completion must seal Complete before reconstruction, acknowledge Store and Validate in order, retire the owner, drain Serve, and only then hand off producer ownership",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                .expect(\"the exact Serve completion must reopen one producer episode\");\n",
            "                ;\n",
            "final Serve retirement must yield the ordinary producer handoff while its live lease rejects duplicate ownership",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "                    .is_err(),\n"
            "                \"one live producer lease must reject a nested ownership claim\"\n",
            "                    .is_ok(),\n"
            "                \"one live producer lease must reject a nested ownership claim\"\n",
            "final Serve retirement must yield the ordinary producer handoff while its live lease rejects duplicate ownership",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_late_passive_fetch_completion_reopens_selected_serve",
            "worker",
            "method",
            "assert_late_passive_fetch_completion_reopens_selected_serve",
            "            drop(producer_episode);\n",
            "            drop(producer_episode);\n"
            "            assert!(\n"
            "                self.services\n"
            "                    .try_begin_certified_serve_producer_episode()\n"
            "                    .expect(\"inspect post-drop producer ownership\")\n"
            "                    .is_none(),\n"
            "                \"the producer debt must be impossible after the first lease drops\"\n"
            "            );\n",
            "integrated late-Fetch producer handoff must make exactly one claim and one nested rejection before dropping the live lease, with no post-drop claim",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_timeout_vote_episode",
            "worker",
            "method",
            "service_timeout_vote_episode",
            "                    FairV2IngressBarrierBypass::TimeoutVoteEpisode,\n",
            "                    FairV2IngressBarrierBypass::None,\n",
            "selected-Serve fixture must use only the reviewed direct TimeoutVote bypass predicate",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_timeout_recovery_prefix",
            "worker",
            "method",
            "service_timeout_recovery_prefix",
            "                        Some(lifecycle_ordinal),\n",
            "                        None,\n",
            "selected-Serve fixture local timeout signature must retain its tracked lifecycle ordinal",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::service_pacemaker",
            "worker",
            "method",
            "service_pacemaker",
            ".step_pacemaker_once(Instant::now(), &mut self.services)",
            ".step(Instant::now(), &mut self.services)",
            "selected-Serve fixture must run exactly one typed pacemaker transition at the live ingress cut",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::entered_view_one",
            "worker",
            "method",
            "entered_view_one",
            "self.executor.current_tag().view() == 1 && self.services.active_tag.view() == 1",
            "self.executor.current_tag().view() == 1",
            "selected-Serve fixture EnterView terminal must agree between reducer and production service",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_complete",
            "worker",
            "method",
            "assert_complete",
            "            assert_eq!(self.remote_timeout_votes_admitted, 2);\n",
            "            assert_eq!(self.remote_timeout_votes_admitted, 1);\n",
            "selected-Serve fixture must retain the Serve and reach exact local plus dual-remote recovery counts",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_complete",
            "worker",
            "method",
            "assert_complete",
            "                            .sum::<usize>()\n"
            "                            == 3\n",
            "                            .sum::<usize>()\n"
            "                            == 2\n",
            "selected-Serve fixture must observe an exact three-signer timeout certificate",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::assert_missing_proposal_serve_selected",
            "worker",
            "method",
            "assert_missing_proposal_serve_selected",
            "            assert_eq!(barrier.request_hash(), self.missing_proposal_request_hash);\n",
            "            assert_ne!(barrier.request_hash(), self.missing_proposal_request_hash);\n",
            "selected-Serve fixture must retain the exact missing-proposal request owner",
        ),
        (
            "worker::SelectedServeTimeoutRecoveryFixture::drop",
            "worker",
            "drop",
            "drop",
            "            drop(self.services.io.take());\n",
            "            let _ = self.services.io.as_ref();\n",
            "selected-Serve synchronous fixture teardown must detach its worker endpoints without a synthetic shutdown",
        ),
    ),
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
            else tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
        )
    )
    worker_test_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "test",
            ")",
            "]",
            "pub",
            "(",
            "super",
            ")",
            "mod",
            "tests",
        ),
    )
    worker_method_context = worker_test_context + (
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
    worker_drop_context = worker_test_context + (
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

def test_selected_serve_timeout_owner_freeze_must_precede_serve_ingress_after_digest_refresh(
    tmp_path: Path,
) -> None:
    """A refreshed fixture seal cannot move height-start timeout ownership after Serve."""

    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    worker_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = worker_path.read_text(encoding="utf-8")
    worker_method_context = (
        (
            "#",
            "[",
            "cfg",
            "(",
            "test",
            ")",
            "]",
            "pub",
            "(",
            "super",
            ")",
            "mod",
            "tests",
        ),
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
