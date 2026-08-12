def test_production_causal_fifo_source_link_rejects_order_and_proof_mutants(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    causal_fifo_errors = module._production_causal_fifo_source_fidelity_errors
    assert causal_fifo_errors(formal_dir) == []
    # Keep this regression scoped to the production causal-FIFO seam; unrelated
    # async source contracts have their own mutation suites.
    module._async_source_fidelity_errors = causal_fifo_errors

    adapter = tmp_path / "crates/iroha_core/src/sumeragi/v2.rs"
    canonical_adapter = adapter.read_text(encoding="utf-8")
    drive_item = module.rust_items(canonical_adapter, "drive_effects")[0]
    drive_start = canonical_adapter.index(drive_item.source)
    drive_end = drive_start + len(drive_item.source)

    def mutate_drive(old: str, new: str) -> str:
        assert drive_item.source.count(old) == 1, old
        return (
            canonical_adapter[:drive_start]
            + drive_item.source.replace(old, new, 1)
            + canonical_adapter[drive_end:]
        )

    def mutate_adapter_item(name: str, old: str, new: str) -> str:
        item = module.rust_items(canonical_adapter, name)[0]
        assert item.source.count(old) == 1, (name, old)
        start = canonical_adapter.index(item.source)
        end = start + len(item.source)
        return (
            canonical_adapter[:start]
            + item.source.replace(old, new, 1)
            + canonical_adapter[end:]
        )

    adapter.write_text(
        mutate_adapter_item(
            "budget",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(1, 4),",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(1, 5),",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "record-specific persistence macro-step budget declaration, contract, "
        "and complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_adapter_item(
            "budget",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(1, 4),",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(2, 4),",
        ),
        encoding="utf-8",
    )
    mutated_budget = module.rust_items(
        adapter.read_text(encoding="utf-8"), "budget"
    )[0]
    module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256["budget"] = (
        module._rust_item_token_sha256(mutated_budget)
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "persistence macro-step budgets must retain the exact reviewed "
        "initial/continuation bounds" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")
    module._PRODUCTION_CAUSAL_FIFO_RUST_ITEM_SHA256["budget"] = (
        module._rust_item_token_sha256(
            module.rust_items(canonical_adapter, "budget")[0]
        )
    )

    for deferred_owner in (
        "            && self.deferred_completions.is_empty()\n",
        "            && self.deferred_progress_inputs.is_empty()\n",
        "            && self.deferred_inputs.is_empty()\n",
    ):
        adapter.write_text(
            mutate_adapter_item("ready_to_finish", deferred_owner, ""),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "terminal adapter deferred-debt readiness fence declaration, "
            "contract, and complete control flow must match" in error
            for error in errors
        ), errors
        adapter.write_text(canonical_adapter, encoding="utf-8")

    deferred_owner_adapter_mutations = (
        (
            "matches_authenticated_runtime_bytes",
            "identity == canonical_bytes",
            "identity != canonical_bytes",
            "exact deferred canonical-envelope comparator declaration, contract, "
            "and complete control flow must match",
        ),
        (
            "deferred_authenticated_message_owner",
            "owned == encoded.as_slice()",
            "owned != encoded.as_slice()",
            "exact Busy-deferred authenticated-envelope owner lookup declaration, contract, and "
            "complete control flow must match",
        ),
        (
            "authenticated_deferred_admission_ordinals",
            ".filter(|input| input.retag_authenticated_ingress)",
            ".filter(|input| !input.retag_authenticated_ingress)",
            "complete authenticated Busy-deferred ordinal snapshot declaration, contract, and "
            "complete control flow must match",
        ),
        (
            "deferred_authenticated_event_matches_wire",
            "message.encode().as_slice() == identity",
            "message.encode().as_slice() != identity",
            "typed deferred event to canonical-envelope comparator declaration, contract, and "
            "complete control flow must match",
        ),
        (
            "wire_ingress_missing_execution_commitment",
            "if vote.validate(&self.wire_context).is_err()",
            "if false",
            "structurally validated missing-execution-commitment ingress classifier declaration, "
            "contract, and complete control flow must match",
        ),
    )
    for item_name, old, new, expected_error in deferred_owner_adapter_mutations:
        adapter.write_text(
            mutate_adapter_item(item_name, old, new),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_adapter_item(
            "drain_deferred_with_evidence",
            "self.drain_deferred_with_evidence_for_ordinals(&eligible)",
            "Ok(None)",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "single-transition adapter deferred ownership dispatcher declaration, contract, "
        "and complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_adapter_item(
            "fail_deferred_service_contract",
            "self.fail_closed = true;",
            "self.fail_closed = false;",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "terminal deferred-service contract failure declaration, contract, and "
        "complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    helper_call = (
        "                    reducer::prepend_causal_continuation("
        "&mut pending, continuation);\n"
    )
    assert canonical_adapter.count(helper_call) == 1
    adapter.write_text(
        canonical_adapter.replace(helper_call, "", 1), encoding="utf-8"
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects must contain exactly one reviewed causal-persistence token sequence"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_drive(
            "        let mut ready = Vec::new();\n",
            "        let mut ready = Vec::new();\n"
            "        if false {\n"
            "            return Ok(Vec::new());\n"
            "        }\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects declaration, contract, and complete control flow must match"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        canonical_adapter.replace(
            helper_call,
            "                    if false {\n"
            + helper_call
            + "                    }\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects declaration, contract, and complete control flow must match"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    runtime = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    canonical_runtime = runtime.read_text(encoding="utf-8")
    canonical_runtime_sources = {runtime: canonical_runtime}
    for component_relative in REVIEWED_RUST_INCLUDE_MANIFESTS[
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs")
    ]:
        component = runtime.parent / component_relative
        canonical_runtime_sources[component] = component.read_text(encoding="utf-8")

    trait_deferred_method = (
        "    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;\n"
    )
    assert canonical_runtime.count(trait_deferred_method) == 1
    runtime.write_text(
        canonical_runtime.replace(
            trait_deferred_method,
            "    #[cfg(test)]\n" + trait_deferred_method,
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "RuntimeDriver authenticated deferred-owner source, snapshots, exact "
        "occurrence ownership, runtime sealing, and exact dispatch methods must "
        "be adjacent on the production trait surface" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    production_driver_context = (
        ("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),
    )
    def write_runtime_item_mutation(name: str, old: str, new: str) -> Path:
        for source_path, canonical_source in canonical_runtime_sources.items():
            items = module.rust_items(canonical_source, name)
            if not items:
                continue
            assert len(items) == 1, (name, source_path)
            item = items[0]
            assert item.source.count(old) == 1, (name, old)
            start = canonical_source.index(item.source)
            end = start + len(item.source)
            source_path.write_text(
                canonical_source[:start]
                + item.source.replace(old, new, 1)
                + canonical_source[end:],
                encoding="utf-8",
            )
            return source_path
        raise AssertionError((name, "reviewed runtime include closure"))

    def restore_runtime_source(source_path: Path) -> None:
        source_path.write_text(
            canonical_runtime_sources[source_path],
            encoding="utf-8",
        )

    def mutate_runtime_item_in_context(
        name: str,
        context: tuple[tuple[str, ...], ...],
        old: str,
        new: str,
    ) -> str:
        items = tuple(
            item
            for item in module.rust_items(canonical_runtime, name)
            if item.brace_context == context
        )
        assert len(items) == 1, (name, context)
        item = items[0]
        assert item.source.count(old) == 1, (name, old)
        start = canonical_runtime.index(item.source)
        end = start + len(item.source)
        return (
            canonical_runtime[:start]
            + item.source.replace(old, new, 1)
            + canonical_runtime[end:]
        )

    runtime_driver_trait_context = (
        ("pub", "(", "crate", ")", "trait", "RuntimeDriver"),
    )
    runtime.write_text(
        mutate_runtime_item_in_context(
            "deferred_occurrence_ownership",
            runtime_driver_trait_context,
            "fn deferred_occurrence_ownership(",
            "fn removed_deferred_occurrence_ownership(",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "exact occurrence ownership, runtime sealing, and exact dispatch methods "
        "must be adjacent" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    trait_occurrence = next(
        item
        for item in module.rust_items(
            canonical_runtime, "deferred_occurrence_ownership"
        )
        if item.brace_context == runtime_driver_trait_context
    )
    trait_seal = next(
        item
        for item in module.rust_items(
            canonical_runtime, "seal_deferred_runtime_ownership"
        )
        if item.brace_context == runtime_driver_trait_context
    )
    occurrence_start = canonical_runtime.index(trait_occurrence.source)
    occurrence_end = occurrence_start + len(trait_occurrence.source)
    seal_start = canonical_runtime.index(trait_seal.source)
    seal_end = seal_start + len(trait_seal.source)
    assert occurrence_end < seal_start
    runtime.write_text(
        canonical_runtime[:occurrence_start]
        + trait_seal.source
        + canonical_runtime[occurrence_end:seal_start]
        + trait_occurrence.source
        + canonical_runtime[seal_end:],
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "exact occurrence ownership, runtime sealing, and exact dispatch methods "
        "must be adjacent" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime_ingress_context = (("impl", "RuntimeIngressOwnershipEvidence"),)
    runtime.write_text(
        mutate_runtime_item_in_context(
            "validate_exact",
            runtime_ingress_context,
            "            && lifecycle_ordinal_is_exact\n"
            "            && leader_wire_runtime_receipt_is_exact\n",
            "            && true\n"
            "            && true\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "canonical ownership validation must include lifecycle and runtime "
        "receipt exactness in its final predicate" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime.write_text(
        mutate_runtime_item_in_context(
            "validate_exact",
            runtime_ingress_context,
            "            (Ok(None), Ok(None)) | (Ok(Some(_)), Ok(None)) | "
            "(Ok(Some(_)), Ok(Some(_)))\n",
            "            (Ok(None), Ok(None)) | (Ok(Some(_)), Ok(Some(_)))\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "canonical ownership validation must bind one lifecycle-ordinal domain "
        "while allowing only the reviewed pre-dequeue leader-wire receipt state"
        in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime.write_text(
        mutate_runtime_item_in_context(
            "validate_frozen_physical",
            runtime_ingress_context,
            "matches!(self.earliest_physical_carrier(), Ok(Some(_)))",
            "self.earliest_physical_carrier().is_ok()",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "post-dequeue ownership validation must require a physical carrier and an "
        "exact token/physical-occurrence/runtime-receipt triple" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime.write_text(
        mutate_runtime_item_in_context(
            "matches_authenticated",
            runtime_ingress_context,
            "self.validate_frozen_physical()",
            "self.validate_exact()",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "authenticated dispatch matching must require the frozen physical "
        "ownership boundary" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    deferred_lifecycle_context = (("impl", "RuntimeDeferredLifecycleOwnership"),)
    runtime.write_text(
        mutate_runtime_item_in_context(
            "validate_active_against_ingress",
            deferred_lifecycle_context,
            "self.runtime_seal.still_retained()",
            "true",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "active deferred lifecycle validation must require a live capability from "
        "the exact adapter source and its frozen ingress" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    production_driver_mutations = (
        (
            "dispatch",
            "if !ownership.matches_authenticated(&message)",
            "if false",
            "production authenticated runtime dispatch bridge declaration and complete control flow must match",
        ),
        (
            "dispatch",
            "tagged.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)",
            "false",
            "authenticated dispatch must bind, clear, and transfer one exact producer lifecycle",
        ),
        (
            "dispatch",
            "self.clear_selected_producer_lifecycle();",
            "",
            "authenticated dispatch must bind, clear, and transfer one exact producer lifecycle",
        ),
        (
            "dispatch",
            "let producer_handoff = outcome.producer_handoff();",
            "let producer_handoff = None;",
            "authenticated dispatch must bind, clear, and transfer one exact producer lifecycle",
        ),
        (
            "dispatch_deferred",
            "SumeragiV2Adapter::drain_deferred_with_handoff_for_ordinals(self, eligible)",
            "Ok(None)",
            "deferred dispatch must retain the selected occurrence and optional producer handoff",
        ),
        (
            "deferred_occurrence_ownership",
            "SumeragiV2Adapter::deferred_occurrence_ownership(self, admission_ordinal)",
            "None",
            "deferred occurrence lookup must preserve the adapter-issued exact occurrence capability",
        ),
        (
            "seal_deferred_runtime_ownership",
            "if !owner.validate_exact()",
            "if false",
            "deferred runtime sealing must validate and bind the exact lifecycle, ingress provenance, physical occurrence, and frozen cut",
        ),
    )
    for item_name, old, new, expected_error in production_driver_mutations:
        runtime.write_text(
            mutate_runtime_item_in_context(
                item_name, production_driver_context, old, new
            ),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        runtime.write_text(canonical_runtime, encoding="utf-8")

    deferred_owner_runtime_mutations = (
        (
            "from_fair_ingress",
            "if outer == *message",
            "if outer != *message",
            "canonical fair-ingress ownership constructor declaration and complete control flow must match",
        ),
        (
            "matches_authenticated",
            "self.runtime_bytes.as_ref() == authenticated.canonical_wire_bytes().as_slice()",
            "self.runtime_bytes.as_ref() != authenticated.canonical_wire_bytes().as_slice()",
            "post-authentication canonical payload comparator declaration and complete control flow must match",
        ),
        (
            "can_merge_downstream",
            "merged.merge_downstream(candidate.clone()).is_ok()",
            "merged.merge_downstream(candidate.clone()).is_err()",
            "non-mutating per-source ownership merge preflight declaration and complete control flow must match",
        ),
        (
            "merge_downstream",
            "self.runtime_bytes != candidate.runtime_bytes",
            "self.runtime_bytes == candidate.runtime_bytes",
            "per-source ownership merge transition declaration and complete control flow must match",
        ),
        (
            "merge_downstream",
            "if retained_lifecycle.is_some() != candidate_lifecycle.is_some()",
            "if false",
            "per-source merge must preserve the tagged-versus-untagged lifecycle domain",
        ),
        (
            "reconcile_deferred_ingress_ownership",
            "if !active.contains(&ordinal) || !candidate.validate_exact()",
            "if active.contains(&ordinal) || !candidate.validate_exact()",
            "authenticated deferred carrier reconciliation declaration and complete control flow must match",
        ),
        (
            "reconcile_deferred_ingress_ownership",
            ".rebase_deferred_ingress(merged_lifecycle, ingress_identity)?",
            ".clone()",
            "an earlier aggregate carrier must rebase its exact deferred owner before either ownership map is committed",
        ),
        (
            "reconcile_deferred_runtime_ownership_after_retirement",
            "self.retire_orphaned_leader_wire_runtime_receipts()",
            "Ok(())",
            "adapter-side retirement reconciliation must validate and prune exact runtime wrappers before terminalizing receipts",
        ),
        (
            "complete_driver_dispatch_leader_wire_owners",
            "self.complete_leader_wire_runtime_owner(parent, handoff)?;",
            "let _ = (parent, handoff);",
            "driver retirement must terminalize the selected parent before orphan receipts",
        ),
        (
            "complete_driver_dispatch_leader_wire_owners",
            "if self.retire_orphaned_leader_wire_runtime_receipts().is_err()",
            "if false",
            "driver retirement must terminalize the selected parent before orphan receipts",
        ),
        (
            "complete_driver_dispatch_leader_wire_owners",
            "        if !retained_parent {\n"
            "            self.complete_leader_wire_runtime_owner(parent, handoff)?;\n"
            "        }\n"
            "        if self.retire_orphaned_leader_wire_runtime_receipts().is_err() {\n",
            "        let orphaned_invalid = self.retire_orphaned_leader_wire_runtime_receipts().is_err();\n"
            "        if !retained_parent {\n"
            "            self.complete_leader_wire_runtime_owner(parent, handoff)?;\n"
            "        }\n"
            "        if orphaned_invalid {\n",
            "driver retirement must terminalize the selected parent before orphan receipts",
        ),
        (
            "accept_driver_dispatch",
            ".reconcile_deferred_ingress_ownership(deferred_ingress)\n            .is_err()",
            ".reconcile_deferred_ingress_ownership(deferred_ingress)\n            .is_ok()",
            "driver dispatch ownership acceptance declaration and complete control flow must match",
        ),
        (
            "accept_driver_dispatch",
            "                || producer_handoff.is_some())",
            "                || false)",
            "retryable dispatch must not expose effects, deferred ownership, or a producer handoff",
        ),
        (
            "accept_driver_dispatch",
            "self.driver.seal_deferred_runtime_ownership(",
            "self.driver.weakened_deferred_runtime_ownership(",
            "driver acceptance must reconcile carrier ownership, seal and verify the exact adapter occurrence",
        ),
        (
            "eligible_deferred_admission_ordinals",
            "u128::from(source_physical_ordinal) >= target.physical_cut",
            "false",
            "deferred eligibility must globally remove post-cut occurrences before choosing the logical minimum",
        ),
        (
            "clock_owner_reservation_blocks_occurrence",
            "u128::from(source_physical_ordinal) >= physical_cut",
            "u128::from(source_physical_ordinal) < physical_cut",
            "post-cut logical replay admission reservation declaration, contract, and complete control flow must match",
        ),
        (
            "clock_owner_reservation_blocks_occurrence",
            "lifecycle_ordinal <= owner.lifecycle_ordinal()",
            "lifecycle_ordinal > owner.lifecycle_ordinal()",
            "post-cut logical replay admission reservation declaration, contract, and complete control flow must match",
        ),
        (
            "enqueue_after_clock_reservation",
            "if self.clock_owner_reservation_blocks(&owner)?",
            "if false",
            "FIFO admission behind immutable clock reservations declaration, contract, and complete control flow must match",
        ),
        (
            "enqueue_after_clock_reservation",
            "return Err(EnqueueError::Full);",
            "return self.ingress.enqueue(command);",
            "post-cut replay must receive recoverable backpressure before FIFO publication",
        ),
        (
            "is_physical_leader_wire_replay",
            "token.admission_ordinal() < physical_ordinal",
            "token.admission_ordinal() <= physical_ordinal",
            "strict retained-token physical-replay classifier declaration and complete control flow must match",
        ),
        (
            "is_physical_leader_wire_replay",
            "token.admission_ordinal() < physical_ordinal",
            "token.scheduler_ordinal() < physical_ordinal",
            "physical replay classification must require a strictly older durable admission token and reject partial ownership",
        ),
        (
            "is_physical_leader_wire_replay",
            "(Some(_), None) | (None, Some(_)) => Err(RuntimeIngressMergeError::Conflict)",
            "(Some(_), None) | (None, Some(_)) => Ok(true)",
            "physical replay classification must require a strictly older durable admission token and reject partial ownership",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "if authenticated_deferred_owner != deferred_owner",
            "if false",
            "authenticated ingress ownership admission and deferred merge declaration and complete control flow must match",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "                    .recognizes_minted(ordinal)\n"
            "                    .unwrap_or(false) => {}",
            "                    .recognizes_minted(ordinal)\n"
            "                    .unwrap_or(true) => {}",
            "authenticated admission must reject an unminted or inconsistent actor-global lifecycle before authentication",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "if blockers.any() && !certified_timeout_escape && !timeout_vote_episode_escape {",
            "if blockers.any() && !certified_timeout_escape {",
            "authenticated ingress ownership admission and deferred merge declaration and complete control flow must match",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "match ingress_ownership.is_physical_leader_wire_replay()",
            "match Ok(true)",
            "only a strict current-round direct-certificate replay or finite TimeoutVote episode owner may cross a clock reservation before FIFO admission",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "self.timeout_recovery_episode_allows_clock_blockers(blockers)",
            "Ok(false)",
            "only a strict current-round direct-certificate replay or finite TimeoutVote episode owner may cross a clock reservation before FIFO admission",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "wire_payload_is_direct_certificate_recovery_shape(\n            authenticated.payload(),\n        )",
            "wire_payload_is_certified_fence_escape(\n            authenticated.payload(),\n        )",
            "only a strict current-round direct-certificate replay or finite TimeoutVote episode owner may cross a clock reservation before FIFO admission",
        ),
        (
            "can_admit_pre_runtime_leader_wire",
            "token.admission_ordinal() < source_physical_ordinal",
            "token.admission_ordinal() <= source_physical_ordinal",
            "pre-runtime admission must use the same strict direct-certificate replay and finite TimeoutVote exceptions as the mutating gate",
        ),
        (
            "can_admit_pre_runtime_leader_wire",
            "wire_payload_is_direct_certificate_recovery_shape(\n                        &runtime_message.payload,\n                    )",
            "wire_payload_is_certified_fence_escape(\n                        &runtime_message.payload,\n                    )",
            "pre-runtime admission must use the same strict direct-certificate replay and finite TimeoutVote exceptions as the mutating gate",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            ".is_some_and(|retained| retained.can_merge_downstream(&ownership))",
            ".is_some()",
            "authenticated ingress ownership capacity preflight declaration and complete control flow must match",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            "                    .recognizes_minted(ordinal)\n"
            "                    .unwrap_or(false)",
            "                    .recognizes_minted(ordinal)\n"
            "                    .unwrap_or(true)",
            "capacity preflight must drain an unminted lifecycle into the mutating fail-closed seam",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            "match self.clock_owner_reservation_blocks_occurrence(",
            "match Ok(false).and(",
            "authenticated ingress ownership capacity preflight declaration and complete control flow must match",
        ),
        (
            "deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences",
            "Err(EnqueueError::Full),",
            "Ok(()),",
            "production causal-FIFO regression deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences declaration, contract, and complete control flow must match",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "assert_eq!(fresh_token.admission_ordinal(), fresh_physical_ordinal);",
            "assert_ne!(fresh_token.admission_ordinal(), fresh_physical_ordinal);",
            "the fresh post-cut carrier must retain equal token and physical ordinals",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "assert!(\n            !runtime.can_admit_network_message_with_ingress_ownership(&message, &fresh_runtime,),",
            "assert!(\n            runtime.can_admit_network_message_with_ingress_ownership(&message, &fresh_runtime,),",
            "the regression must reject the fresh certified carrier",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "Err(NetworkIngressError::Backpressure(EnqueueError::Full))",
            "Ok(runtime.round_tag())",
            "the mutating seam must backpressure a fresh certified carrier",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "!runtime.fail_closed,\n            \"rejecting the fresh carrier is retryable backpressure\"",
            "runtime.fail_closed,\n            \"rejecting the fresh carrier is retryable backpressure\"",
            "fresh certified backpressure must remain recoverable",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "fresh_runtime_projection.is_physical_leader_wire_replay(),\n            Ok(false),",
            "fresh_runtime_projection.is_physical_leader_wire_replay(),\n            Ok(true),",
            "the fresh case must exercise the strict physical-replay helper",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "            Ok(true),\n            \"the fresh carrier must exercise the active timeout reservation\"",
            "            Ok(false),\n            \"the fresh carrier must exercise the active timeout reservation\"",
            "the fresh negative case must exercise an active clock reservation",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "assert_eq!(runtime.queued_commands(), queued_before_fresh);",
            "assert_eq!(runtime.queued_commands(), queued_before_fresh + 1);",
            "fresh backpressure must publish no queue, receipt, or terminal state",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "restored_receipt.token().admission_ordinal() < restored_physical_ordinal",
            "restored_receipt.token().admission_ordinal() <= restored_physical_ordinal",
            "the regression must admit only a strictly later retained replay",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "assert!(\n            runtime\n                .can_admit_network_message_with_ingress_ownership(&message, &restored_pre_runtime,),",
            "assert!(\n            !runtime\n                .can_admit_network_message_with_ingress_ownership(&message, &restored_pre_runtime,),",
            "the regression must admit the retained replay through fair ingress",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "restored_runtime_projection.is_physical_leader_wire_replay(),\n            Ok(true),",
            "restored_runtime_projection.is_physical_leader_wire_replay(),\n            Ok(false),",
            "the retained case must exercise the strict physical-replay helper",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "            Ok(true),\n            \"the restored carrier must exercise the narrow replay exception\"",
            "            Ok(false),\n            \"the restored carrier must exercise the narrow replay exception\"",
            "the positive replay case must exercise the active reservation exception",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            ".is_some_and(|queued| queued.restored_producer_stage.is_none()),",
            ".is_some_and(|queued| queued.restored_producer_stage.is_some()),",
            "the replay must publish exactly one ordinary authenticated Admit owner",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "RuntimeSelectedOwnerKind::Timeout",
            "RuntimeSelectedOwnerKind::Fifo",
            "the frozen timeout must retain the first serialized turn",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "[AdapterEffect::EnterView { tag, .. }] if tag.view() == 1",
            "[AdapterEffect::EnterView { tag, .. }] if tag.view() == 0",
            "the restored TC must advance the view after the timeout turn",
        ),
        (
            "restored_pre_runtime_tc_cannot_deadlock_a_newly_frozen_timeout_owner",
            "terminals.len(),\n            1,",
            "terminals.len(),\n            0,",
            "the restored TC lifecycle must terminalize exactly once",
        ),
        (
            "take_last_scheduler_ownership",
            "self.last_scheduler_ownership.take()",
            "self.last_scheduler_ownership.clone()",
            "runner scheduler ownership handoff declaration and complete control flow must match",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "fn fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers()",
            "fn removed_fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers()",
            "named fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers; found 0",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "fn fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers()",
            "fn real_adapter_fence_completion_breaks_pre_and_post_timeout_retransmit_debt()",
            "retired production regression real_adapter_fence_completion_breaks_pre_and_post_timeout_retransmit_debt is prohibited",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "                .expect(\"freeze the pre-deadline second retransmission\"),\n"
            "            RuntimeStep::Idle",
            "                .expect(\"freeze the pre-deadline second retransmission\"),\n"
            "            RuntimeStep::Advanced(Vec::new())",
            "a fresh pre-timeout periodic episode must remain at the runtime boundary behind an older signer without creating adapter debt",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "assert_eq!(prepare_completion.selected, RuntimeSelectedOwnerKind::Fifo);",
            "assert_eq!(prepare_completion.selected, RuntimeSelectedOwnerKind::FenceCompletion);",
            "the older Prepare completion must retain ordinary FIFO ownership without a fence predecessor or dependency bypass",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "assert!(!prepare_completion.fence_completion_bypass);",
            "assert!(prepare_completion.fence_completion_bypass);",
            "the older Prepare completion must retain ordinary FIFO ownership without a fence predecessor or dependency bypass",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "                .expect(\"freeze post-timeout retransmission behind signing\"),\n"
            "            RuntimeStep::Idle",
            "                .expect(\"freeze post-timeout retransmission behind signing\"),\n"
            "            RuntimeStep::Advanced(Vec::new())",
            "a fresh post-timeout periodic episode must remain at the runtime boundary behind TimeoutVote signing without creating adapter debt",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "assert_eq!(timeout_completion.selected, RuntimeSelectedOwnerKind::Fifo);",
            "assert_eq!(timeout_completion.selected, RuntimeSelectedOwnerKind::FenceCompletion);",
            "the older TimeoutVote completion must retain ordinary FIFO ownership without a fence predecessor or dependency bypass",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "assert!(!timeout_completion.fence_completion_bypass);",
            "assert!(timeout_completion.fence_completion_bypass);",
            "the older TimeoutVote completion must retain ordinary FIFO ownership without a fence predecessor or dependency bypass",
        ),
        (
            "fresh_periodic_episodes_wait_behind_pre_and_post_timeout_signers",
            "            RuntimeSelectedOwnerKind::PeriodicTimer\n"
            "        );",
            "            RuntimeSelectedOwnerKind::Fifo\n"
            "        );",
            "the retained post-timeout periodic episode must run after the older completion, clear, and leave later periodic ticks armed",
        ),
        (
            "with_driver_and_lifecycle_ordinals",
            "        ingress\n"
            "            .install_dormant_local_fifo_reservations("
            "dormant_local_fifo_reservations)\n"
            "            .map_err(|_| "
            "RuntimeConfigError::InvalidLifecycleOwnership)?;",
            "        let _ = dormant_local_fifo_reservations;",
            "restart must install dormant Local FIFO reservations before retaining any startup successor",
        ),
        (
            "finish_dispatched_step",
            "if token.identity().admission_ordinal() != "
            "effect_parent.lifecycle_ordinal()",
            "if false",
            "live dispatch completion must retain successors, acknowledge the exact producer, terminalize the selected parent before adapter-side orphans",
        ),
        (
            "step_recovery",
            "if token.identity().admission_ordinal() != owner.lifecycle_ordinal()",
            "if false",
            "recovery dispatch must retain successors, acknowledge the exact producer, terminalize the selected parent before adapter-side orphans",
        ),
        (
            "dispatch_one_adapter_deferred",
            "if token.identity().admission_ordinal() != "
            "lifecycle_owner.lifecycle_ordinal()",
            "if false",
            "deferred dispatch must retain successors, acknowledge the exact producer, terminalize the selected parent before adapter-side orphans",
        ),
        (
            "try_step_pacemaker_escape",
            "if timeout_due {",
            "if false {",
            "typed pacemaker escape must prefer the absolute timeout and otherwise admit only Progress-root work",
        ),
        (
            "dispatch_one_pacemaker_progress",
            "owner.causal_origin().root_class == SERVICE_CLASS_PROGRESS",
            "owner.causal_origin().root_class != SERVICE_CLASS_PROGRESS",
            "pacemaker FIFO escape must retain exact selection evidence and Progress-root ownership through shared completion",
        ),
        (
            "dispatch_one_pacemaker_progress",
            "driver.certified_progress_bypasses_signature_fence(command)",
            "false",
            "pacemaker FIFO escape must retain exact selection evidence and Progress-root ownership through shared completion",
        ),
        (
            "later_same_semantic_fair_retry_retains_runtime_lifecycle_root",
            "fn later_same_semantic_fair_retry_retains_runtime_lifecycle_root()",
            "fn removed_later_same_semantic_fair_retry_retains_runtime_lifecycle_root()",
            "named later_same_semantic_fair_retry_retains_runtime_lifecycle_root; found 0",
        ),
        (
            "later_same_semantic_fair_retry_retains_runtime_lifecycle_root",
            "assert_eq!(queued.lifecycle_ordinal, Some(retained_ordinal));",
            "assert_eq!(queued.lifecycle_ordinal, Some(retry_ordinal));",
            "same-semantic retry must preserve its first immutable runtime lifecycle root",
        ),
        (
            "ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it",
            "fn ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it()",
            "fn removed_ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it()",
            "named ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it; found 0",
        ),
        (
            "ordinary_fair_predecessor_remains_before_serve_until_runtime_consumes_it",
            "assert_eq!(consumed.lifecycle_ordinal, fair_ordinal);",
            "assert_eq!(consumed.lifecycle_ordinal, serve_ordinal);",
            "ordinary Fair ownership must precede Serve exactly until runtime consumes it",
        ),
        (
            "older_frozen_aggregate_carrier_rebases_queued_runtime_minimum",
            "fn older_frozen_aggregate_carrier_rebases_queued_runtime_minimum()",
            "fn removed_older_frozen_aggregate_carrier_rebases_queued_runtime_minimum()",
            "named older_frozen_aggregate_carrier_rebases_queued_runtime_minimum; found 0",
        ),
        (
            "older_frozen_aggregate_carrier_rebases_queued_runtime_minimum",
            "assert_eq!(queued.lifecycle_ordinal, Some(older_ordinal));",
            "assert_eq!(queued.lifecycle_ordinal, Some(newer_ordinal));",
            "an older aggregate carrier must become the queued runtime minimum",
        ),
        (
            "network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals",
            "fn network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals()",
            "fn removed_network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals()",
            "named network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals; found 0",
        ),
        (
            "network_runtime_rejects_unminted_and_unrelated_colliding_fair_ordinals",
            "assert_eq!(unminted_runtime.queued_commands(), 0);",
            "assert_eq!(unminted_runtime.queued_commands(), 1);",
            "unminted fair ownership must fail closed without a physical queue position",
        ),
        (
            "certified_tc_crosses_full_fence_blocked_prepare_prefix",
            "            Some(0),",
            "            None,",
            "Busy-deferred authenticated response coalescing regression declaration and complete control flow must match",
        ),
    )
    for item_name, old, new, expected_error in deferred_owner_runtime_mutations:
        mutated_path = write_runtime_item_mutation(item_name, old, new)
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        restore_runtime_source(mutated_path)

    mutated_path = write_runtime_item_mutation(
        "dispatch_one_adapter_deferred",
        "if !self.driver.deferred_work_is_serviceable()",
        "if self.driver.deferred_work_is_serviceable()",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "single adapter-deferred runtime dispatcher declaration, contract, and "
        "complete control flow must match" in error
        for error in errors
    ), errors
    restore_runtime_source(mutated_path)

    mutated_path = write_runtime_item_mutation(
        "step",
        "        if !timeout_preempts\n"
        "            && let Some(step) = self.dispatch_one_adapter_deferred(now, None)?\n"
        "        {\n",
        "        if false {\n"
        "            return Ok(RuntimeStep::Idle);\n"
        "        }\n"
        "        if !timeout_preempts\n"
        "            && let Some(step) = self.dispatch_one_adapter_deferred(now, None)?\n"
        "        {\n",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "live serialized runtime step declaration, contract, and complete "
        "control flow must match" in error
        for error in errors
    ), errors
    restore_runtime_source(mutated_path)

    refinement = (
        tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    )
    canonical_refinement = refinement.read_text(encoding="utf-8")
    refinement.write_text(
        canonical_refinement.replace("#[allow(dead_code)]\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel must have exact reviewed attributes"
        in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    assert canonical_refinement.count("continuation.into_iter().rev()") == 1
    refinement.write_text(
        canonical_refinement.replace(
            "continuation.into_iter().rev()", "continuation.into_iter()", 1
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation reverse-iteration/push-front FIFO kernel "
        "must match the exact reviewed Rust/Verus item body" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        canonical_refinement.replace("pending.push_front(item)", "pending.push_back(item)", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation reverse-iteration/push-front FIFO kernel "
        "must match the exact reviewed Rust/Verus item body" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    helper_start = canonical_refinement.index(
        "pub fn prepend_causal_continuation<T>("
    )
    helper_end = canonical_refinement.index(
        "\n}\n\n/// Caller-visible reducer action classes", helper_start
    ) + 2
    helper_source = canonical_refinement[helper_start:helper_end]
    refinement.write_text(
        canonical_refinement[:helper_start]
        + canonical_refinement[helper_end:]
        + "\nmacro_rules! stuffed_helper {\n"
        + "    () => {\n"
        + helper_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel must have reviewed brace context" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    for opener, closer in (("(", ")"), ("[", "]")):
        refinement.write_text(
            canonical_refinement[:helper_start]
            + canonical_refinement[helper_end:]
            + f"\nstuffed_helper!{opener}\n"
            + helper_source
            + f"\n{closer};\n",
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "prepend_causal_continuation kernel must have reviewed all-delimiter context"
            in error
            for error in errors
        ), (opener, errors)
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        canonical_refinement.replace(
            "pub fn prepend_causal_continuation<T>(",
            "#[cfg(any())]\npub fn prepend_causal_continuation<T>(",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel may not be disabled or replaced" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        "#![cfg(any())]\n" + canonical_refinement,
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    core = tmp_path / "crates/iroha_core/src/sumeragi/v2_core.rs"
    canonical_core = core.read_text(encoding="utf-8")
    export_start = canonical_core.index("pub(crate) use refinement::{")
    export_end = canonical_core.index("\n};", export_start) + 3
    export_source = canonical_core[export_start:export_end]
    core.write_text(
        canonical_core[:export_start]
        + canonical_core[export_end:]
        + "\nmacro_rules! stuffed_refinement_export {\n"
        + "    () => {\n"
        + export_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one direct top-level pub(crate) use refinement::{...} "
        "export; found 0" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    for opener, closer in (("(", ")"), ("[", "]")):
        core.write_text(
            canonical_core[:export_start]
            + canonical_core[export_end:]
            + f"\nstuffed_refinement_export!{opener}\n"
            + export_source
            + f"\n{closer};\n",
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "require exactly one direct top-level pub(crate) use refinement::{...} "
            "export; found 0" in error
            for error in errors
        ), (opener, errors)
    core.write_text(canonical_core, encoding="utf-8")

    core.write_text(
        canonical_core.replace(
            "pub(crate) use refinement::{",
            "#[cfg(any())]\npub(crate) use refinement::{",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one direct top-level pub(crate) use refinement::{...} "
        "export; found 0" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    core.write_text("#![cfg(any())]\n" + canonical_core, encoding="utf-8")
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "real top-level refinement export may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    verus = tmp_path / "crates/iroha_sumeragi_core/src/verus_proofs.rs"
    canonical_verus = verus.read_text(encoding="utf-8")
    theorem_start = canonical_verus.index(
        "pub proof fn production_reverse_push_front_refines_fifo("
    )
    theorem_end = canonical_verus.index(
        "\n\n/// Stable first-owner filter", theorem_start
    )
    verus.write_text(
        canonical_verus[:theorem_start]
        + "/*\n"
        + canonical_verus[theorem_start:theorem_end]
        + "\n*/\n"
        + canonical_verus[theorem_end + 2 :],
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one real Rust/Verus function item named "
        "production_reverse_push_front_refines_fifo; found 0" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    completeness_item = module.rust_items(
        canonical_verus,
        "production_fresh_causal_successors_keeps_every_fresh_value",
    )[0]
    weakened_completeness = completeness_item.source.replace(
        "successors.contains(candidate) && !owned.contains(candidate)",
        "false",
        1,
    )
    assert weakened_completeness != completeness_item.source
    verus.write_text(
        canonical_verus.replace(
            completeness_item.source,
            weakened_completeness,
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_keeps_every_fresh_value declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    assert canonical_verus.count("if owned.contains(candidate) {") >= 3
    verus.write_text(
        canonical_verus.replace(
            "if owned.contains(candidate) {",
            "if !owned.contains(candidate) {",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "stable first-owner causal-successor filter must match the exact reviewed"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    unique_item = module.rust_items(
        canonical_verus,
        "production_fresh_causal_successors_has_unique_values",
    )[0]
    weakened_unique = unique_item.source.replace(
        "production_fresh_causal_successors(owned, successors).no_duplicates(),",
        "production_fresh_causal_successors(owned, successors).len() >= 0,",
        1,
    )
    assert weakened_unique != unique_item.source
    verus.write_text(
        canonical_verus.replace(unique_item.source, weakened_unique, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_has_unique_values declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    proof_open = unique_item.source.find("{")
    assert proof_open > 0
    empty_unique = unique_item.source[:proof_open] + "{/* old proof body */}"
    verus.write_text(
        canonical_verus.replace(unique_item.source, empty_unique, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_has_unique_values declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    reverse_theorem_source = canonical_verus[theorem_start:theorem_end]
    verus.write_text(
        canonical_verus[:theorem_start]
        + canonical_verus[theorem_end:]
        + "\nmacro_rules! stuffed_verus_theorem {\n"
        + "    () => {\n"
        + reverse_theorem_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production reverse-push-front FIFO theorem must have reviewed brace context"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    verus.write_text(
        canonical_verus.replace(
            "pub proof fn production_reverse_push_front_refines_fifo(",
            "#[cfg(any())]\n"
            "pub proof fn production_reverse_push_front_refines_fifo(",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production reverse-push-front FIFO theorem may not be disabled or replaced"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    adapter.write_text(
        canonical_adapter.replace(
            "#[cfg(test)]\nmod tests {\n",
            "#[cfg(test)]\nmod tests {\n    #![cfg(any())]\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "strengthened TC-order regression may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    tc_name = "fn tc_promoted_lock_requires_same_subject_reproposal_before_commit()"
    tc_start = canonical_adapter.index(tc_name)
    no_sign_start = canonical_adapter.index("        assert!(\n            installed", tc_start)
    match_start = canonical_adapter.index(
        "        let fetch_tag = match installed.as_slice() {", no_sign_start
    )
    match_end = canonical_adapter.index(
        "\n\n        assert!(matches!(\n            adapter\n"
        "                .body_available(fetch_tag, manifest)",
        match_start,
    )
    exact_match = canonical_adapter[match_start:match_end]
    commit_witness_start = canonical_adapter.index(
        "        let validation = adapter", match_end
    )
    tc_test_end = canonical_adapter.index(
        "\n    }\n\n    #[test]", commit_witness_start
    )
    tc_source = canonical_adapter[tc_start:tc_test_end]

    def mutate_tc(old: str, new: str) -> str:
        assert tc_source.count(old) == 1, old
        return (
            canonical_adapter[:tc_start]
            + tc_source.replace(old, new, 1)
            + canonical_adapter[tc_test_end:]
        )

    tc_mutations = (
        (
            mutate_tc(
                tc_name + " {",
                tc_name + " {\n        if false { return; }",
            ),
            "strengthened TC regression declaration and complete control flow "
            "must match the exact reviewed token digest",
        ),
        (
            canonical_adapter[:no_sign_start]
            + canonical_adapter[match_start:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + "        let fetch_tag = installed\n"
            "            .iter()\n"
            "            .find_map(|effect| match effect {\n"
            "                AdapterEffect::FetchBody { tag, .. } => Some(*tag),\n"
            "                _ => None,\n"
            "            })\n"
            "            .expect(\"fetch body\");"
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + exact_match.replace(
                "AdapterEffect::EnterView", "AdapterEffect::__SWAP", 1
            )
            .replace("AdapterEffect::FetchBody", "AdapterEffect::EnterView", 1)
            .replace("AdapterEffect::__SWAP", "AdapterEffect::FetchBody", 1)
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + exact_match.replace(
                "                },\n            ] if enter_tag == tag",
                "                },\n                ..\n            ] if enter_tag == tag",
                1,
            )
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            mutate_tc(
                "                && *fetched_subject == subject",
                "                && *fetched_subject != subject",
            ),
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            mutate_tc(
                "            }] if *tag == fetch_tag\n"
                "                && *stored_round == round",
                "            }] if *tag == timeout_tag\n"
                "                && *stored_round == round",
            ),
            "TC regression must pin exact StoreBody/ValidateBody tags, rounds, and subjects",
        ),
        (
            mutate_tc(
                "            }] if *tag == fetch_tag\n"
                "                && *validated_round == round",
                "            }] if *tag == timeout_tag\n"
                "                && *validated_round == round",
            ),
            "TC regression must pin exact StoreBody/ValidateBody tags, rounds, and subjects",
        ),
        (
            mutate_tc(
                "validation.is_empty()",
                "!validation.is_empty()",
            ),
            "TC regression must pin the post-validation no-Commit boundary, WAL, "
            "and status witness",
        ),
        (
            mutate_tc(
                ".validation_succeeded(fetch_tag, round, subject, &validated)",
                ".validation_succeeded(fetch_tag, round, subject, &other)",
            ),
            "strengthened TC regression must contain exactly one "
            "adapter.validation_succeeded(fetch_tag, round, subject, &validated)",
        ),
        (
            mutate_tc(
                "            adapter.wal.recovered_records().len(),\n"
                "            2,\n",
                "            adapter.wal.recovered_records().len(),\n"
                "            3,\n",
            ),
            "TC regression must pin the post-validation no-Commit boundary, WAL, "
            "and status witness",
        ),
        (
            mutate_tc(
                "            .commit_intent(core_current_round),\n"
                "            None,",
                "            .commit_intent(core_current_round),\n"
                "            Some(reducer::Vote::new(round, subject, 0)),",
            ),
            "TC regression must pin the post-validation no-Commit boundary, WAL, "
            "and status witness",
        ),
        (
            mutate_tc(
                "wire::SumeragiV2OutboundIntentKind::CommitQc",
                "wire::SumeragiV2OutboundIntentKind::PrepareQc",
            ),
            "TC regression must pin the post-validation no-Commit boundary, WAL, "
            "and status witness",
        ),
    )
    for mutated_adapter, expected_error in tc_mutations:
        adapter.write_text(mutated_adapter, encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
    adapter.write_text(canonical_adapter, encoding="utf-8")
    verus.write_text(canonical_verus, encoding="utf-8")

    stable_prepend = (
        "seq![candidate].add(production_fresh_causal_successors(\n"
        "                owned.insert(candidate),\n"
        "                remaining,\n"
        "            ))"
    )
    stable_reverse = (
        "production_fresh_causal_successors(\n"
        "                owned.insert(candidate),\n"
        "                remaining,\n"
        "            ).add(seq![candidate])"
    )
    assert canonical_verus.count(stable_prepend) == 1
    verus.write_text(
        canonical_verus.replace(stable_prepend, stable_reverse, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "stable first-owner causal-successor filter must match the exact reviewed"
        in error
        for error in errors
    ), errors
