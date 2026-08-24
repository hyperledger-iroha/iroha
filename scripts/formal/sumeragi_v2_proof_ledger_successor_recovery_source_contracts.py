# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _successor_recovery_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind recovered-height storage, lifecycle, and ingress sources."""

    errors: list[str] = []

    def load(relative: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(repo_root, relative, errors, "production successor-refinement source")

    def region(path: Path, source: str, label: str, start_marker: str, end_marker: str) -> str:
        start = source.find(start_marker)
        end = source.find(end_marker, start + len(start_marker)) if start >= 0 else -1
        if start < 0 or end < 0:
            errors.append(f"{path}: missing exact production region {label}")
            return ""
        return source[start:end]

    def require_tokens(path: Path, label: str, body: str, tokens: tuple[str, ...]) -> None:
        body_tokens = rust_code_tokens(body)
        missing = [
            token
            for token in tokens
            if _token_sequence_count(body_tokens, rust_code_tokens(token)) == 0
        ]
        if missing:
            errors.append(
                f"{path}: {label} omits production refinement tokens {missing}"
            )

    def require_token_count(
        path: Path, label: str, body: str, token: str, expected: int,
    ) -> None:
        observed = _token_sequence_count(rust_code_tokens(body), rust_code_tokens(token))
        if observed != expected:
            errors.append(
                f"{path}: {label} must contain {token!r} exactly {expected} "
                f"time(s); found {observed}"
            )

    def require_literal_count(path: Path, label: str, body: str, literal: str, expected: int) -> None:
        observed = mask_rust_comments(body).count(literal)
        if observed != expected:
            errors.append(
                f"{path}: {label} must contain exact production literal "
                f"{literal!r} exactly {expected} time(s); found {observed}"
            )

    def require_order(path: Path, label: str, body: str, markers: tuple[str, ...]) -> None:
        body_tokens = rust_code_tokens(body)
        cursor = 0
        for marker in markers:
            marker_tokens = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(
                        cursor,
                        len(body_tokens) - len(marker_tokens) + 1,
                    )
                    if body_tokens[index : index + len(marker_tokens)] == marker_tokens
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{path}: {label} must preserve exact production order {markers}"
                )
                return
            cursor = position + len(marker_tokens)

    def reject_tokens(path: Path, label: str, body: str, forbidden: tuple[str, ...]) -> None:
        body_tokens = rust_code_tokens(body)
        observed = tuple(
            token
            for token in forbidden
            if _token_sequence_count(body_tokens, rust_code_tokens(token))
        )
        if observed:
            errors.append(
                f"{path}: {label} must use the opaque checked-transition gate; "
                f"found obsolete direct-kernel forms {observed}"
            )
    runner_path, runner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    lifecycle_run_inner_path, lifecycle_run_inner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"
    )
    lifecycle_pending_kura_path, lifecycle_pending_kura_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
    )
    runner_authority_path, runner_authority_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs"
    )
    sumeragi_path, sumeragi_source = load(
        "crates/iroha_core/src/sumeragi/mod.rs"
    )

    recovery_path, recovery_source = load(
        "crates/iroha_core/src/sumeragi/v2_recovery.rs"
    )
    if recovery_source:
        production_recovery_source = recovery_source.split(
            "\n#[cfg(test)]\nmod tests {", 1
        )[0]
        predecessor_authentication = region(
            recovery_path,
            recovery_source,
            "DurableV2PredecessorIdentity::authenticate",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "\n    /// Lossless primitive identity consumed by the shared production/Verus kernel.",
        )
        require_tokens(
            recovery_path,
            "DurableV2PredecessorIdentity::authenticate",
            predecessor_authentication,
            (
                "height: artifact.height, block_hash: artifact.block_hash, artifact_hash: HashOf::new(artifact),",
                "receipt.height() != identity.height || receipt.block_hash() != identity.block_hash || receipt.context_id() != artifact.context_id() || receipt.subject() != artifact.subject || receipt.certificate() != artifact.commit_qc.as_ref() || receipt.artifact_hash() != identity.artifact_hash",
                "if !production_durable_predecessor_identity_kernel(identity.refinement_projection())",
            ),
        )
        require_order(
            recovery_path,
            "DurableV2PredecessorIdentity::authenticate",
            predecessor_authentication,
            (
                "let identity = Self",
                "receipt.height() != identity.height",
                "production_durable_predecessor_identity_kernel(identity.refinement_projection())",
                "Ok(identity)",
            ),
        )
        complete_tip_authority = region(
            recovery_path,
            recovery_source,
            "RecoveredCompleteTipActivationAuthority",
            "pub(crate) struct RecoveredCompleteTipActivationAuthority {",
            "\n/// Distinct one-shot authority for the first executable height after an audited snapshot.",
        )
        require_tokens(
            recovery_path,
            "RecoveredCompleteTipActivationAuthority canonical lifecycle target",
            complete_tip_authority,
            (
                "verified_predecessor: VerifiedHeightContext",
                "predecessor_signature_policy: BlockSignaturePolicy",
                "lifecycle_storage: CanonicalCompleteTipLifecycleStorageV1",
                "struct CanonicalLifecycleHeightStorageV1",
                "kura.sumeragi_v2_storage_root().join(\"lifecycle-v1\").join(hex::encode(context_id.0.as_ref()))",
                "CanonicalCompleteTipLifecycleStorageV1::from_kura( kura, artifact.context_id(), artifact.height, verified_successor.context().id(), verified_successor.context().height, )",
                "verified_predecessor.context() != &artifact.height_context",
                "verified_predecessor.proofs_of_possession() != artifact.validator_set_pops.as_slice()",
                "self.lifecycle_storage.predecessor.root == root",
                "self.lifecycle_storage.successor.context_id == self.activation.successor_context_id()",
                "body_store_root: kura.sumeragi_v2_storage_root().join(\"bodies\")",
                "fn authorizes_predecessor_storage_inputs(",
                "self.lifecycle_storage.body_store_root == body_store_root",
                "&self.predecessor_signature_policy == signature_policy",
                "fn into_canonical_predecessor_storage(",
                "fn authorizes_verified_successor(",
                "verified.context().parent_commit_qc.as_ref() == Some(&self.artifact.commit_qc)",
                "verified.verified_predecessor_context() == Some(&self.artifact.height_context)",
                "fn authorizes_successor_body_store(",
            ),
        )
        require_tokens(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            recovery_source,
            (
                "lifecycle_storage_authority: RecoveredLifecycleStorageAuthorityV1",
                "RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(",
                "struct RecoveredLifecycleStorageMintPermitV1",
                "genesis_account: AccountId",
                "fn authorizes(",
                "self.kura_identity.matches(kura)",
                "&self.genesis_account == genesis_account",
                "RecoveredLifecycleStorageMintPermitV1::new(",
                "self.lifecycle_storage_authority",
            ),
        )
        require_token_count(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            production_recovery_source,
            "RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(",
            5,
        )
        require_token_count(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            production_recovery_source,
            "RecoveredLifecycleStorageMintPermitV1::new(",
            5,
        )
        successor_storage_projection = _require_rust_item(
            recovery_path,
            production_recovery_source,
            "into_parts_with_lifecycle_storage_authority",
            errors,
        )
        if successor_storage_projection is not None:
            require_order(
                recovery_path,
                "verified successor lifecycle storage authority projection",
                successor_storage_projection.source,
                (
                    "let Self { verified_context, activation, kura_identity, } = self",
                    "if !kura_identity.matches(kura)",
                    "V2RecoveryError::SuccessorLifecycleStorageKuraMismatch",
                    "let signature_policy = BlockSignaturePolicy::RotatingLeader",
                    "RecoveredLifecycleStorageMintPermitV1::new( kura, &verified_context, &signature_policy, genesis_account, )",
                    "RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height( kura, &verified_context, &signature_policy, genesis_account, permit, )",
                    "Ok(( verified_context, activation, lifecycle_storage_authority ))",
                ),
            )
        require_tokens(
            recovery_path,
            "verified successor exact Kura retention",
            production_recovery_source,
            (
                "struct VerifiedSuccessorHeight",
                "kura_identity: KuraInstanceIdentity",
                "kura_identity: state.kura().instance_identity()",
            ),
        )
        require_token_count(
            recovery_path,
            "verified successor exact Kura retention",
            production_recovery_source,
            "kura_identity: state.kura().instance_identity()",
            2,
        )
        successor_storage_behavior = _require_rust_item(
            recovery_path,
            recovery_source,
            "verified_successor_projects_only_its_exact_kura_lifecycle_storage",
            errors,
        )
        if successor_storage_behavior is not None:
            require_order(
                recovery_path,
                "verified successor lifecycle storage projection behavior",
                successor_storage_behavior.source,
                (
                    "let successor = build_verified_successor(&state, &store, &artifact, &receipt)",
                    ".into_parts_with_lifecycle_storage_authority(kura.as_ref(), &genesis_account)",
                    "assert_eq!(successor.context().id(), successor_context_id)",
                    "let foreign_kura = Kura::blank_kura_for_testing()",
                    "let successor = build_verified_successor(&state, &store, &artifact, &receipt)",
                    "successor.into_parts_with_lifecycle_storage_authority( foreign_kura.as_ref(), &genesis_account, )",
                    "Err(V2RecoveryError::SuccessorLifecycleStorageKuraMismatch { height: 2 })",
                ),
            )
        require_token_count(
            runner_path,
            "runner successor lifecycle storage authority consumption",
            f"{runner_source}\n{lifecycle_run_inner_source}\n{lifecycle_pending_kura_source}",
            "into_parts_with_lifecycle_storage_authority(",
            2,
        )
        require_tokens(
            runner_path,
            "runner retains recovered lifecycle storage authority",
            runner_source,
            (
                "lifecycle_storage_authority",
                "first_height_authenticated_genesis",
                "match pending_kura_apply",
                "lifecycle_run_inner::run_non_pending_lifecycle_loop(",
                "lifecycle_pending_kura::run_pending_kura_lifecycle_height(",
            ),
        )
        recover_active_height = _require_rust_item(
            recovery_path,
            production_recovery_source,
            "recover_active_height_with_plan",
            errors,
        )
        if recover_active_height is not None:
            require_tokens(
                recovery_path,
                "recovery-sealed fresh genesis handoff",
                recover_active_height.source,
                (
                    "let (verified_context, staged_genesis_nexus_amx_context, authenticated_genesis) = fresh_genesis.into_parts()",
                    "if !authenticated_genesis.authorizes(&genesis_public_key)",
                    "FreshGenesisAuthorityMismatch",
                    "authenticated_genesis: Some(authenticated_genesis)",
                ),
            )
        require_tokens(
            recovery_path,
            "recovery-sealed fresh genesis owner",
            production_recovery_source,
            (
                "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>",
                "self.authenticated_genesis",
            ),
        )
        ledger_path, ledger_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs"
        )
        ledger_operations_path, ledger_operations_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs"
        )
        if ledger_source:
            predecessor_store_join = region(
                ledger_path,
                ledger_source,
                "CompleteTip canonical predecessor store join",
                "fn is_authorized_complete_tip_predecessor_target(",
                "\n    /// Compare the complete immutable publication target",
            )
            require_tokens(
                ledger_path,
                "CompleteTip canonical predecessor store join",
                ledger_source,
                (
                    "complete_tip.authorizes_predecessor_lifecycle_root(root)",
                    "self.path == root.join(LEDGER_FILE)",
                    "pub(in crate::sumeragi) fn open_complete_tip_predecessor_storage(",
                    "complete_tip.authorizes_predecessor_storage_inputs(",
                    "CertifiedServePayloadStoreV1::open( predecessor_root, verified_predecessor.context() )?",
                    "recovered.authenticate_for_complete_tip_retirement( &verified_predecessor, local_signer )?",
                    "authenticate_complete_tip_serve_census( &terminal.ledger, &serve_payloads )?",
                    "payload_store.retire_authenticated_cut(serve_payloads, &retained_serve_payloads)?",
                    "reconcile_complete_tip_serve_retirement(",
                    ".stage_finalized_height_all_row_retirement(serve_reconciliation)?",
                    ".persist_exact_successor(&terminal.ledger, &retired)?",
                    "successor.open_initialized_or_descendant(retired.high_water())?",
                    "RetiredRecoveredCompleteTipActivationAuthorityV1",
                    "predecessor_store: LifecycleLedgerStoreV1",
                    "predecessor_ledger: LifecycleLedgerV1",
                    "successor_store: LifecycleLedgerStoreV1",
                    "successor_ledger: LifecycleLedgerV1",
                    "fn bind_successor_owner(",
                    "owner_store.same_publication_target(&self.successor_store)",
                    "LifecycleLedgerV1::from_coordinator(&owner.coordinator)",
                    "authorizes_successor_body_store(body_store, &owner.verified)",
                    "owner.payload_store.matches_lifecycle_storage_root(",
                    "owner.payload_store.validate_authenticated_cut(&owner.serve_payloads)",
                    "authenticated_serve_payloads_match_ledger( successor_ledger, &owner.serve_payloads, )",
                    "adapter_startup.authorizes_verified_context(&owner.verified)",
                    "self.complete_tip.authorizes_successor_kura(owner.kura_binding.as_ref())",
                    "serve_payloads: recovery.into_serve_payloads()",
                ),
            )
            restart_publication = region(
                ledger_path,
                ledger_source,
                "CompleteTip restart publication authority",
                "fn successor_descends_from_retirement(",
                "\n    fn matches_successor_owner_ledger(",
            )
            require_order(
                ledger_path,
                "CompleteTip restart publication authority",
                restart_publication,
                (
                    "self.successor_ledger.frame_identity() == self.successor_frame_identity",
                    "self.frame_descends_from_retained_floor(&self.successor_ledger)",
                    "fn predecessor_remains_exact(&self) -> bool",
                    "self.predecessor_ledger.frame_identity() == self.predecessor_frame_identity",
                    ".is_authorized_complete_tip_predecessor_target(&self.complete_tip)",
                    "self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)",
                    "fn authorizes_owner_open_successor(&self, successor: &LifecycleLedgerV1) -> bool",
                    "successor == &self.successor_ledger",
                    "self.successor_descends_from_retirement()",
                    "self.successor_ledger.records.is_empty()",
                    "self.successor_ledger.producer_debts.is_empty()",
                    "self.successor_ledger.high_water == self.retained_high_water",
                    "record.ordinal() > self.retained_high_water",
                    "fn authorizes_retained_successor(&self) -> bool",
                    "self.predecessor_remains_exact()",
                    "self.successor_descends_from_retirement()",
                    "self.complete_tip.authorizes_successor_lifecycle_target(",
                    "self.successor_store.load().ok().as_ref() == Some(&self.successor_ledger)",
                    "fn authorizes_successor_status(",
                    "self.authorizes_retained_successor()",
                    "self.complete_tip.successor_context_id() == successor.height_context_id",
                    ".checked_add(1)",
                    "Some(successor.height)",
                    "successor.last_committed_height == self.complete_tip.predecessor().height()",
                ),
            )
            reject_tokens(
                ledger_path,
                "CompleteTip restart publication authority",
                restart_publication,
                (
                    "#[cfg(test)]",
                    "into_parts",
                    "fn root(",
                    "fn ledger(",
                ),
            )
            successor_owner_bind = region(
                ledger_path,
                ledger_source,
                "CompleteTip exact H+1 owner bind",
                "fn matches_successor_owner_ledger(",
                "\n/// Private Kura-derived target for the empty CompleteTip successor ledger.",
            )
            require_order(
                ledger_path,
                "CompleteTip exact H+1 owner bind",
                successor_owner_bind,
                (
                    "fn matches_successor_owner_ledger(",
                    "self.predecessor_remains_exact()",
                    "authorizes_successor_kura(owner.kura_binding.as_ref())",
                    "authorizes_verified_successor(&owner.verified)",
                    "authorizes_successor_lifecycle_target(successor_root, successor_ledger.context())",
                    "authorizes_successor_body_store(body_store, &owner.verified)",
                    "adapter_startup.authorizes_verified_context(&owner.verified)",
                    "matches_lifecycle_storage_root(successor_root, owner.verified.context())",
                    "validate_authenticated_cut(&owner.serve_payloads)",
                    "authenticated_serve_payloads_match_ledger( successor_ledger, &owner.serve_payloads, )",
                    "owner_store.same_publication_target(&self.successor_store)",
                    "self.successor_store.load().ok().as_ref() != Some(successor_ledger)",
                    "owner_store.load().ok().as_ref() != Some(successor_ledger)",
                    "LifecycleLedgerV1::from_coordinator(&owner.coordinator)",
                    "owner.exact_lifecycle_output_ordinals_for_registry_census()",
                    "exactly_covers_recovered_ready_work_with_owner_held_outputs(",
                    "fn exactly_matches_successor_owner(",
                    "self.successor_descends_from_retirement()",
                    "self.matches_successor_owner_ledger(owner, &self.successor_ledger)",
                    "fn bind_successor_owner( mut self, mut owner: ProductionLifecycleOwnerV1, )",
                    "self.successor_store.load()",
                    "self.authorizes_owner_open_successor(&successor_ledger)",
                    "successor.authorizes_complete_tip_owner_join(",
                    "self.matches_successor_owner_ledger(&mut owner, &successor_ledger)",
                    "owner.timeout_supersession_successor.take()",
                    "self.successor_frame_identity = successor_ledger.frame_identity()",
                    "self.successor_ledger = successor_ledger",
                    "self.exactly_matches_successor_owner(&mut owner)",
                    "BoundRecoveredCompleteTipSuccessorOwnerV1 { owner, retirement: self, }",
                    "struct BoundRecoveredCompleteTipSuccessorOwnerV1 { owner: ProductionLifecycleOwnerV1, retirement: RetiredRecoveredCompleteTipActivationAuthorityV1, }",
                    "impl BoundRecoveredCompleteTipSuccessorOwnerV1",
                    "fn launch( self, inputs: super::launch::ProductionLifecycleLaunchInputsV1, )",
                    "let Self { owner, retirement } = self",
                    "let launched = owner.launch(inputs)?",
                    "LaunchedRecoveredCompleteTipSuccessorLifecycleV1 { launched, retirement, }",
                    "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 { launched: Box<super::launch::LaunchedProductionLifecycleV1>, retirement: RetiredRecoveredCompleteTipActivationAuthorityV1, }",
                    "impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
                    "fn initialize_recovered_local_proposal( &mut self, runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1, )",
                    "self.launched.initialize_recovered_local_proposal(runner)",
                    "fn activate( self, now: std::time::Instant, runner: super::super::v2_runner::ProductionLifecycleCompleteTipRunnerActivationV1, local_proposal: super::launch::ProductionLifecyclePreparedLocalProposalStateV1, )",
                    "let Self { launched, retirement, } = self",
                    "launched.activate_recovered_complete_tip(now, runner, retirement, local_proposal)",
                ),
            )
            reject_tokens(
                ledger_path,
                "CompleteTip exact H+1 owner bind",
                successor_owner_bind,
                (
                    "into_parts",
                    "fn into_owner(",
                    "-> ProductionLifecycleOwnerV1",
                    "root(&self)",
                    "ledger(&self)",
                    "fn owner(&self)",
                    "fn retirement(&self)",
                    "fn launched(&self)",
                    "fn into_launched(",
                    "fn into_retirement(",
                ),
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 bound seal",
                ledger_source,
                "BoundRecoveredCompleteTipSuccessorOwnerV1",
                5,
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 bound seal",
                ledger_source,
                "impl BoundRecoveredCompleteTipSuccessorOwnerV1",
                2,
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 launched seal",
                ledger_source,
                "LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
                4,
            )

        adapter_path, adapter_source = load(
            "crates/iroha_core/src/sumeragi/v2.rs"
        )
        body_store_path, body_store_source = load(
            "crates/iroha_core/src/sumeragi/v2_body_store.rs"
        )
        safety_wal_path, safety_wal_source = load(
            "crates/iroha_core/src/sumeragi/safety_wal.rs"
        )
        adjacent_store_path, adjacent_store_source = load(
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs"
        )
        if adapter_source:
            authenticated_startup = region(
                adapter_path,
                adapter_source,
                "authenticated recovered startup projections",
                "pub(crate) fn authenticate_final_wal_startup_authority(",
                "impl AuthenticatedRecoveredAdapterStartup",
            )
            require_order(
                adapter_path,
                "authenticated recovered startup projections",
                authenticated_startup,
                (
                    "authenticate_recovered_wal_frontier()",
                    "recovered_validation_authority(&self.effects)",
                    "authenticate_recovered_wal_vote_sign(&mut self.effects)",
                    "authenticate_recovered_wal_control_sign(&mut self.effects)",
                    "authenticate_recovered_wal_decision_fetch(&mut self.effects)",
                ),
            )
            require_tokens(
                adapter_path,
                "test-only authenticated recovered startup projections",
                adapter_source,
                (
                    "validation_authority: RecoveredValidationAuthority",
                    "#[cfg(test)] const fn recovered_validation_authority(",
                    "#[cfg(test)] fn leader_wire_recovery_authority(",
                    "struct ProductionLeaderWireLaunchAuthorityV1",
                    "fn prepare_leader_wire_launch(",
                ),
            )
            canonical_owner_factory = region(
                adapter_path,
                adapter_source,
                "canonical Kura-bound lifecycle-owner factory",
                "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
                "fn open_production_lifecycle_owner_v1_at_authenticated_roots(",
            )
            require_order(
                adapter_path,
                "canonical Kura-bound lifecycle-owner factory",
                canonical_owner_factory,
                (
                    "factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1",
                    "body_store: super::v2_body_store::QuarantinedV2BodyStore",
                    "if !self.effects.is_empty()",
                    "let RecoveredLifecycleOwnerFactoryInputsV1 { adapter_owner, storage, state, queue, kura, provider_ingest_finalized_archive, reputation_finalized_archive, block_cadence, events_sender, local_signer, } = factory_inputs",
                    "Arc::ptr_eq(&adapter_owner, &self.factory_owner)",
                    "storage.context_id != context.id() || storage.height != context.height",
                    "body_store.matches_lifecycle_storage_root( &storage.body_store_root, &context, &storage.signature_policy, )",
                    "self.adapter.wal.matches_path(&storage.wal_path)",
                    "let apply_service = super::v2_apply::V2ApplyService::new(",
                    "storage.genesis_account.clone()",
                    "apply_service.matches_lifecycle_launch( &state, &kura, &context, &validator_set_pops )",
                    "body_store.into_revalidated_lifecycle_startup( &apply_service, &context, validation_authority )",
                    "let RecoveredLifecycleStorageAuthorityV1 { kura_identity, wal_path, chunk_root, lifecycle_root, successor_floor, .. } = storage",
                    "self.open_production_lifecycle_owner_v1_at_authenticated_roots(",
                    "let owner = match successor_floor",
                    "owner.authenticate_recovered_successor_floor(floor)",
                    "let kura_binding = RecoveredLifecycleOwnerKuraBindingV1 { kura_identity, wal_path, chunk_root, local_signer: Some(local_signer.public_key().clone()), }",
                    "owner.with_recovered_kura_binding_and_apply_service(kura_binding, apply_service)",
                ),
            )
            reject_tokens(
                adapter_path,
                "canonical Kura-bound lifecycle-owner factory",
                canonical_owner_factory,
                (
                    "kura: &Kura",
                    "ledger_root: &std::path::Path",
                    "serve_payload_root: &std::path::Path",
                    "body_root: &std::path::Path",
                    "body_signature_policy:",
                    "body_store: super::v2_body_store::V2BodyStore",
                    "body_store: super::v2_body_store::RevalidatedV2BodyStore",
                    "state.sumeragi_block_cadence()",
                ),
            )
            require_tokens(
                adapter_path,
                "recovery-minted lifecycle storage authority",
                adapter_source,
                (
                    "struct RecoveredLifecycleStorageAuthorityV1",
                    "kura_identity: KuraInstanceIdentity",
                    "genesis_account: AccountId",
                    "wal_path: PathBuf",
                    "chunk_root: PathBuf",
                    "struct RecoveredLifecycleOwnerKuraBindingV1 {",
                    "fn matches_identity(&self, identity: &KuraInstanceIdentity) -> bool",
                    "fn storage_paths_for_launch(",
                    "struct RecoveredLifecycleOwnerFactoryInputsV1",
                    "adapter_owner: Arc<AuthenticatedRecoveredAdapterFactoryOwnerV1>",
                    "fn bind_production_lifecycle_owner_factory_inputs_v1(",
                    "permit: super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "storage.kura_identity.matches(kura.as_ref())",
                    "state.matches_kura_instance(&kura)",
                    "state.network_id_ref() != &self.adapter.wire_context.network_id",
                    "let (local_signer, block_cadence) = permit.into_factory_dependencies()",
                    "fn mint_from_recovered_height(",
                    "permit: super::v2_recovery::RecoveredLifecycleStorageMintPermitV1",
                    "assert!(permit.authorizes(kura, verified, signature_policy, genesis_account))",
                    "let storage_root = kura.sumeragi_v2_storage_root()",
                    "kura_identity: kura.instance_identity()",
                    "wal_path: storage_root .join(\"wal\") .join(format!(\"{:020}.wal\", context.height))",
                    "chunk_root: storage_root.join(\"chunks\")",
                    "lifecycle_root: storage_root .join(\"lifecycle-v1\") .join(hex::encode(context.id().0.as_ref()))",
                    "body_store_root: storage_root.join(\"bodies\")",
                    "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
                    "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network()",
                    "fn recovered_lifecycle_factory_inputs_reject_a_same_context_foreign_startup()",
                    "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
                    "assert!(context_binding < body_root)",
                    "assert!(body_root < wal_path)",
                    "assert!(wal_path < apply_service)",
                    "assert!(authenticated_roots < kura_binding)",
                ),
            )
            factory_dependency_bind = region(
                adapter_path,
                adapter_source,
                "authenticated lifecycle factory cadence",
                "fn bind_production_lifecycle_owner_factory_inputs_v1(",
                "/// Consume all recovered adapter and storage authority",
            )
            require_order(
                adapter_path,
                "authenticated lifecycle factory cadence",
                factory_dependency_bind,
                (
                    "state.network_id_ref() != &self.adapter.wire_context.network_id",
                    "let (local_signer, block_cadence) = permit.into_factory_dependencies()",
                    "RecoveredLifecycleOwnerFactoryInputsV1 {",
                    "block_cadence",
                ),
            )
            reject_tokens(
                adapter_path,
                "authenticated lifecycle factory cadence",
                factory_dependency_bind,
                ("state.sumeragi_block_cadence()",),
            )
            activation_behavior = _require_rust_item(
                adapter_path,
                adapter_source,
                "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout",
                errors,
            )
            if activation_behavior is not None:
                require_order(
                    adapter_path,
                    "production lifecycle activation behavior",
                    activation_behavior.source,
                    (
                        "let mut launched = owner.launch(launch_inputs)",
                        "assert!(crate::sumeragi::status::v2_status().is_none())",
                        "let mut setup_runner = super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1::for_test()",
                        "let mut activation = super::super::v2_runner::ProductionLifecycleRunnerActivationV1::current_height_for_test(",
                        ".with_canonical_body_recovery_ingress( &mut setup_runner, &mut activation,",
                        "assert!(ingress_ready.load(Ordering::Acquire))",
                        "assert!(leader_wire_ingress.state.lock().open)",
                        "assert!(!ingress_ready.load(Ordering::Acquire))",
                        "assert!(!leader_wire_ingress.state.lock().open)",
                        ".with_runner_setup(&mut setup_runner",
                        "retain_recovered_local_proposal_attempt_for_test(recovered_attempt)",
                        "let (joined_directive, local_proposal_state) = launched .initialize_recovered_local_proposal(setup_runner)",
                        "assert!(local_proposal_state.already_attempted(directive))",
                        "let mut activated = launched .activate(Instant::now(), activation, local_proposal_state)",
                        "assert!(ingress_ready.load(Ordering::Acquire))",
                        "assert!(leader_wire_ingress.state.lock().open)",
                        "activated.with_runner_runtime(",
                        ".retire_lifecycle_stores_for_test(finality_receipt)",
                        "cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)",
                        "assert!(!ingress_ready.load(Ordering::Acquire))",
                        "assert!(!leader_wire_ingress.state.lock().open)",
                    ),
                )
            finalization_behavior = _require_rust_item(
                adapter_path,
                adapter_source,
                "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
                errors,
            )
            if finalization_behavior is not None:
                require_order(
                    adapter_path,
                    "production lifecycle finalization behavior",
                    finalization_behavior.source,
                    (
                        "let _status_guard = crate::sumeragi::status::rbc_status_test_guard()",
                        "Algorithm::Ed25519",
                        "TransactionBuilder::new_genesis(",
                        "block_builder.set_da_proof_policies(Some(proof_policy_bundle))",
                        ".try_build_with_signature(0, genesis_key.private_key())",
                        "BlockSignaturePolicy::GenesisAuthority(",
                        "WalRecordV2::Decision(decision.clone())",
                        "let owner = result.unwrap_or_else",
                        "let mut lane_work = super::super::v2_lane_work::V2LaneWorkAdapter::lifecycle_finalization_fixture_for_test(",
                        "let mut launched = owner.launch(launch_inputs)",
                        "let mut setup_runner = ProductionLifecyclePreActivationRunnerBorrowV1::for_test()",
                        ".with_runner_setup(&mut setup_runner",
                        "services.set_exact_output_admission_hook(|_post, _ticket| Ok(()))",
                        "launched.drive_completion_turn_for_test(runner, &mut lane_work)",
                        "ProductionCompletionDispatchV1::ApplyQueued",
                        "launched.drive_completion_turn_for_test(runner, &mut lane_work)",
                        "ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied",
                        ".initialize_recovered_local_proposal(setup_runner)",
                        "let mut activated = launched .activate(Instant::now(), activation, local_proposal_state)",
                        "drop(auxiliary_hold)", "ProductionLifecycleIngressSelectionV1::CertifiedServeQueued",
                        "assert_eq!(leader_wire_ingress.len(), 0)",
                        ".reconcile_decided_lane_certified_serve(&mut serve_runner, permit)",
                        "if completed",
                        "ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady",
                        "let claimed_producer = activated .claim_producer_turn_for_local_proposal(&mut serve_runner)",
                        "let attempted_producer = claimed_producer .into_attempted(producer_turn_attempt_permit_for_test(&mut serve_runner))",
                        "activated .settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
                        "ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued",
                        "ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted",
                        "!selected.restart_required()",
                        "let mut runner = super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1::for_test()",
                        "super::super::v2_runner::lifecycle_run_inner::finalize_lifecycle_height(",
                        "assert_eq!(receipt.context_id(), recovered_context.id())",
                        "assert_eq!(artifact.subject, subject)",
                        ".retain_merge_sidecars_for_global_view(",
                        "let mut successor = recovered_context.clone()",
                        "Ok::<_, super::super::v2_runner::V2RunnerError>((successor, ()))",
                        "drop(retained_sidecars)",
                        "assert!(outcome.cleanup().warnings().is_empty())",
                        "assert!(outcome.wal_retirement_warning().is_none())",
                    ),
                )
                admitted_serve_dispatch = region(
                    adapter_path, finalization_behavior.source,
                    "direct lifecycle Certified-Serve dispatch",
                    "drop(auxiliary_hold);", "let claimed_producer = activated",
                )
                reject_tokens(
                    adapter_path, "direct lifecycle Certified-Serve dispatch",
                    admitted_serve_dispatch, (
                        "consume_prepared_ordinary_ingress_turn(", "has_prepared_serve_for_test()",
                    ),
                )
            for literal in (
                '"a caller-promoted marker cannot enter production quarantine"',
                '"pre-promoted marker rejection must precede lifecycle-store creation"',
                '"a body store outside the Kura layout must fail closed"',
                '"a wrong body signature policy must fail closed"',
            ):
                require_literal_count(
                    adapter_path,
                    "recovery-minted lifecycle storage authority regressions",
                    adapter_source,
                    literal,
                    1,
                )
            reject_tokens(
                adapter_path,
                "sealed recovered lifecycle factory inputs",
                adapter_source,
                (
                    "fn genesis_account_for_launch(",
                    "impl Clone for RecoveredLifecycleOwnerFactoryInputsV1",
                    "impl Clone for AuthenticatedRecoveredAdapterStartup",
                ),
            )
            require_tokens(
                adapter_path,
                "factory-retained local signer identity",
                canonical_owner_factory,
                (
                    "local_signer",
                    "&local_signer",
                    "local_signer: Some(local_signer.public_key().clone())",
                ),
            )
            reject_tokens(
                adapter_path,
                "factory-retained local signer identity",
                canonical_owner_factory,
                ("local_signer: &KeyPair",),
            )
        if body_store_source:
            require_tokens(
                body_store_path,
                "fresh quarantined recovered body-store cut",
                body_store_source,
                (
                    "struct QuarantinedV2BodyStore(V2BodyStore)",
                    "fn into_quarantined_recovered_startup(",
                    "!self.validated.is_empty() || !self.rejected.is_empty() || !self.retired_revalidation.is_empty()",
                    "V2BodyStoreError::RecoveredMarkersAlreadyPromoted",
                ),
            )
            quarantine = region(
                body_store_path,
                body_store_source,
                "fixed quarantined recovered marker replay",
                "impl QuarantinedV2BodyStore {",
                "impl RevalidatedV2BodyStore {",
            )
            require_order(
                body_store_path,
                "fixed quarantined recovered marker replay",
                quarantine,
                (
                    "fn into_revalidated_lifecycle_startup(",
                    "apply_service.recovered_finality_subject(context)",
                    "self.0.retain_recovered_markers_for_subject(subject)",
                    "self.0.retain_recovered_markers_for_authority(validation_authority)",
                    "self.0.revalidate_recovered_markers(|body|",
                    "apply_service.revalidate_recovered_candidate(context, body)",
                    "self.0.into_revalidated_startup()",
                ),
            )
            reject_tokens(
                body_store_path,
                "fixed quarantined recovered marker replay",
                quarantine,
                (
                    "pub(in crate::sumeragi) fn retain_recovered_markers_for_subject(",
                    "pub(in crate::sumeragi) fn retain_recovered_markers_for_authority(",
                    "pub(in crate::sumeragi) fn revalidate_recovered_markers<",
                    "pub(in crate::sumeragi) fn into_revalidated_startup(",
                ),
            )
            require_tokens(
                body_store_path,
                "revalidated body-store canonical-root oracle",
                body_store_source,
                (
                    "fn matches_lifecycle_storage_root(",
                    "&self.0.signature_policy == signature_policy",
                    "self.0.directory == root.join(hex::encode(context.id().0.as_ref()))",
                    "StoreRootMismatch",
                ),
            )
            terminal_store_join = region(
                ledger_path,
                ledger_source,
                "CompleteTip terminal Apply store join",
                "fn into_complete_tip_terminal_apply_store_join(",
                "\n    /// Purely stage one adapter-authenticated WAL-ahead Validate-to-Sign repair.",
            )
            require_order(
                ledger_path,
                "CompleteTip terminal Apply store join",
                terminal_store_join,
                (
                    "ledger_store.is_authorized_complete_tip_predecessor_target(&complete_tip)",
                    "ledger_store.context != self.context()",
                    "ledger_store.load()? != self",
                    "predecessor_evidence.exactly_matches(&ledger_store, &self, &complete_tip)",
                    "predecessor_evidence",
                    "cut.is_exact()?",
                    "Ok(cut)",
                ),
            )
            require_tokens(
                ledger_path,
                "CompleteTip foreign target regression",
                ledger_source,
                (
                    "fn complete_tip_terminal_apply_store_join_rejects_an_identical_foreign_target()",
                    "fn complete_tip_all_row_retirement_is_exact_and_restart_idempotent()",
                    "fn complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work()",
                    "fn complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner()",
                ),
            )
        launch_path, launch_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
        )
        turn_driver_path, turn_driver_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs"
        )
        kura_path, kura_source = load("crates/iroha_core/src/kura.rs")
        owner_path, owner_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs"
        )
        ledger_store_path, ledger_store_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs"
        )
        worker_path, worker_source = load(
            "crates/iroha_core/src/sumeragi/v2_worker.rs"
        )
        scheduler_path, scheduler_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs"
        )
        registry_path, registry_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs"
        )
        registry_validate_path, registry_validate_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs"
        )
        registry_validate_impl_path, registry_validate_impl_source = load(
            "crates/iroha_core/src/sumeragi/"
            "v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"
        )
        concrete_admission_path, concrete_admission_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_concrete_admission.rs"
        )
        lifecycle_projection_path, lifecycle_projection_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs"
        )
        wal_recovery_path, wal_recovery_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs"
        )
        selector_path, selector_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs"
        )
        ingress_position_path, ingress_position_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs"
        )
        body_pipeline_path, body_pipeline_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs"
        )
        replay_authority_path, replay_authority_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs"
        )
        runtime_path, runtime_source = load(
            "crates/iroha_core/src/sumeragi/v2_runtime.rs"
        )
        effects_path, effects_source = load(
            "crates/iroha_core/src/sumeragi/v2_effects.rs"
        )
        transport_path, transport_source = load(
            "crates/iroha_core/src/sumeragi/v2_transport.rs"
        )
        lifecycle_open_path, lifecycle_open_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs"
        )
        runner_dependency_path = runner_authority_path
        runner_dependency_source = f"{runner_authority_source}\n{runner_source}"
        finalized_output_path, finalized_output_source = load(
            "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs"
        )
        lifecycle_startup_test_path, lifecycle_startup_test_source = load(
            "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs"
        )
        state_path, state_source = load("crates/iroha_core/src/state.rs")
        snapshot_path, snapshot_source = load("crates/iroha_core/src/snapshot.rs")
        schema_path, schema_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs"
        )
        apply_path, apply_source = load(
            "crates/iroha_core/src/sumeragi/v2_apply.rs"
        )
        _successor_recovery_lifecycle_source_fidelity_errors(
            adapter_path,
            adapter_source,
            adjacent_store_path,
            adjacent_store_source,
            apply_path,
            apply_source,
            body_pipeline_path,
            body_pipeline_source,
            body_store_path,
            body_store_source,
            concrete_admission_path,
            concrete_admission_source,
            effects_path,
            effects_source,
            errors,
            finalized_output_source,
            ingress_position_path,
            ingress_position_source,
            kura_path,
            kura_source,
            launch_path,
            launch_source,
            ledger_operations_path,
            ledger_operations_source,
            ledger_path,
            ledger_source,
            ledger_store_path,
            ledger_store_source,
            lifecycle_open_path,
            lifecycle_open_source,
            lifecycle_projection_path,
            lifecycle_projection_source,
            lifecycle_startup_test_source,
            owner_path,
            owner_source,
            region,
            registry_path,
            registry_source,
            registry_validate_impl_path,
            registry_validate_impl_source,
            registry_validate_path,
            registry_validate_source,
            reject_tokens,
            replay_authority_path,
            replay_authority_source,
            require_literal_count,
            require_order,
            require_token_count,
            require_tokens,
            runner_dependency_path,
            runner_dependency_source,
            runtime_path,
            runtime_source,
            safety_wal_path,
            safety_wal_source,
            scheduler_path,
            scheduler_source,
            schema_path,
            schema_source,
            selector_path,
            selector_source,
            snapshot_path,
            snapshot_source,
            state_path,
            state_source,
            sumeragi_path,
            sumeragi_source,
            transport_path,
            transport_source,
            turn_driver_path,
            turn_driver_source,
            wal_recovery_path,
            wal_recovery_source,
            worker_path,
            worker_source,
        )
        payload_store_path, payload_store_source = load(
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs"
        )
        coordinator_path, coordinator_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs"
        )
        if payload_store_source and lifecycle_open_source and coordinator_source:
            payload_census = region(
                payload_store_path,
                payload_store_source,
                "CompleteTip Serve payload directory census",
                "fn reload_payload_census_strict(",
                "\n    /// Verify that a post-authentication startup cut still covers the complete",
            )
            require_order(
                payload_store_path,
                "CompleteTip Serve payload directory census",
                payload_census,
                (
                    "fs::symlink_metadata(&self.directory)",
                    "directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir()",
                    "self.max_entries.checked_mul(2)",
                    "fs::read_dir(&self.directory)",
                    "fs::symlink_metadata(&path)",
                    "metadata.file_type().is_symlink() || !metadata.is_file()",
                    "!has_canonical_hash_name(name, FILE_SUFFIX)",
                    "payloads.len() >= self.max_entries",
                    "self.load_path(&path, metadata.len())?",
                    "self.path_for(payload.id()) != path",
                    "payloads.insert(payload.id(), payload).is_some()",
                    "Ok(payloads)",
                ),
            )
            require_tokens(
                payload_store_path,
                "CompleteTip body-independent Completed metadata authority",
                payload_store_source,
                (
                    "fn authenticate_for_complete_tip_retirement(",
                    ".certificate .signers .binary_search(&persisted_responder)",
                    "body_revalidated: body_store.is_some()",
                    "fn permits_payload_store_ahead_terminal_rebind(&self) -> bool",
                    "fn retirement_rejects_completed_metadata_from_a_noncertified_responder()",
                    "fn reload_payload_census_strict(",
                    "let observed = self.reload_payload_census_strict()?",
                    "observed_ids != self.indexed || cut_ids != observed_ids",
                    "fn authenticated_cut_rejects_a_later_valid_payload_from_a_second_store_owner()",
                    "fn authenticated_cut_rejects_store_directory_symlink_replacement()",
                ),
            )
            payload_authentication = _require_rust_item(
                payload_store_path,
                payload_store_source,
                "authenticate_inner",
                errors,
            )
            if payload_authentication is not None:
                require_literal_count(
                    payload_store_path,
                    "CompleteTip body-independent Completed metadata authority",
                    payload_authentication.source,
                    '"persisted response signer lost certified local retention authority"',
                    1,
                )
            require_tokens(
                lifecycle_open_path,
                "CompleteTip bodyless completion promotion guard",
                lifecycle_open_source,
                (
                    "completed.permits_payload_store_ahead_terminal_rebind()",
                    "pub(super) fn into_serve_payloads(self)",
                    "authenticated_serve_payloads_match_ledger(",
                ),
            )
            require_tokens(
                coordinator_path,
                "production lifecycle owner retained Serve census",
                coordinator_source,
                (
                    "struct ProductionLifecycleOwnerV1",
                    "serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut",
                    "fn run_complete_tip_retirement_release_regressions()",
                    "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work()",
                    "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner()",
                ),
            )
            current_retirement_census = region(
                payload_store_path,
                payload_store_source,
                "live Serve retirement directory authentication",
                "fn authenticate_current_for_lifecycle_retirement(",
                "/// Compare this opened payload owner",
            )
            require_order(
                payload_store_path,
                "live Serve retirement directory authentication",
                current_retirement_census,
                (
                    "verified.context() != &self.context",
                    "self.reload_payload_census_strict()?",
                    "payloads.keys().copied().collect::<BTreeSet<_>>() != self.indexed",
                    "CertifiedServePayloadRecoveryCut {",
                    ".authenticate_for_complete_tip_retirement(verified, local_signer)",
                    "self.validate_authenticated_cut(&authenticated)?",
                    "Ok(authenticated)",
                ),
            )
            live_serve_join = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "live finalization Serve ledger/admission-wait join",
                "fn authenticate_live_finalization_serve_census(",
                "/// Seal the final post-mutation Serve cut",
            )
            require_order(
                lifecycle_open_path,
                "live finalization Serve ledger/admission-wait join",
                live_serve_join,
                (
                    "LifecycleLedgerV1::from_coordinator(coordinator)",
                    "authenticate_complete_tip_serve_census(ledger, recovered)?",
                    "WaitSource::Capacity(class)",
                    "receipt.exactly_matches_pending(payload.request())",
                    "prepare_certified_serve_admission(",
                    "candidate != waiting.candidate",
                    "owned != recovered_ids",
                    "Ok(retained)",
                ),
            )
            launch_serve_refresh = region(
                launch_path,
                launch_source,
                "launched live Serve retirement refresh",
                "fn refresh_live_serve_retirement_cut(",
                "/// Cross the ordinary/current/snapshot live-height boundary",
            )
            require_order(
                launch_path,
                "launched live Serve retirement refresh",
                launch_serve_refresh,
                (
                    "_retired_ingress: &ProductionLifecycleRetiredIngressPermitV1",
                    "exactly_covers_finalization_work(&self.coordinator)",
                    "authenticate_current_lifecycle_serve_retirement(",
                    "LifecycleLedgerV1::from_coordinator(&self.coordinator)",
                    "authenticate_live_finalization_serve_census(",
                    "self.serve_payloads = refreshed",
                ),
            )
            reject_tokens(
                launch_path,
                "launched live Serve retirement refresh",
                launch_serve_refresh,
                (
                    "CertifiedServePayloadStoreV1::open(",
                    "KeyPair::from_private_key",
                    "impl Clone for ProductionLifecycleServeRetirementAuthenticationPermitV1",
                ),
            )
            fixture_retirement = region(
                launch_path,
                launch_source,
                "consuming activated Serve retirement fixture",
                "fn retire_lifecycle_stores_for_test(",
                "pub(in crate::sumeragi) fn with_runner_runtime<R>(",
            )
            require_order(
                launch_path,
                "consuming activated Serve retirement fixture",
                fixture_retirement,
                (
                    "let Self { mut launched, local_proposal, runner_activation, } = self",
                    "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
                    "drop(local_proposal)",
                    "launched.leader_wire_ingress_binding.retire()",
                    "seal_empty_exact_output_for_lifecycle_retirement_test()",
                    "refresh_live_serve_retirement_cut(&launched.services, &retired_ingress)",
                    ".retire_lifecycle_stores()",
                ),
            )
            finalization_readiness = region(
                launch_path,
                launch_source,
                "shared lifecycle finalization readiness",
                "fn ready_for_finalized_rollover(&mut self) -> bool {",
                "impl ActivatedProductionLifecycleV1",
            )
            require_order(
                launch_path,
                "shared lifecycle finalization readiness",
                finalization_readiness,
                (
                    "self.executor.ready_to_finish()",
                    "!self.owner.has_recovered_lifecycle_outputs()",
                    "self.pending_kura_apply_replay.is_none()",
                    "self.recovered_local_proposal_attempt.is_none()",
                    "self.pending_lifecycle_completion.is_none()",
                    "self.pending_ingress_capacity.is_none()",
                    "self.completion_observer_activation.is_none()",
                    "exactly_covers_finalization_work(&self.owner.coordinator)",
                ),
            )
            activated_finalization = region(
                launch_path,
                launch_source,
                "activated lifecycle finalization",
                "fn into_finalized_rollover(",
                "pub(in crate::sumeragi) fn retire_lifecycle_stores_for_test(",
            )
            require_order(
                launch_path,
                "activated lifecycle finalization",
                activated_finalization,
                (
                    "self.launched.ready_for_finalized_rollover()",
                    "let Self { mut launched, local_proposal, runner_activation, } = self",
                    "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
                    "drop(local_proposal)",
                    "launched.leader_wire_ingress_binding.retire()",
                    "executor.into_finalized_parts()",
                    "begin_fail_stop_operation()",
                    "runtime.into_driver().finish_height(&receipt, &artifact)",
                    "operation.complete()",
                    "FinalizedProductionLifecycleRolloverV1 {",
                ),
            )
            require_tokens(
                launch_path,
                "activated lifecycle finalization quiescence",
                activated_finalization,
                (
                    "ProductionLifecycleFinalizationErrorV1::NotReady",
                    "finalized_adapter: finalized",
                ),
            )
            require_order(
                lifecycle_run_inner_path,
                "runner lifecycle finalization preflight",
                lifecycle_run_inner_source,
                (
                    "if !ready_to_finish || producer_turn.is_some()",
                    "schedule_local_proposal(",
                    "let finalization_ready = ready_to_finish && activated.ready_for_finalized_rollover(&mut active_runner)",
                    "let rollover_ready = if finalization_ready",
                    "preflight_finalized_lane_rollover(",
                    "if ready_to_finish && !rollover_ready",
                    "finalize_lifecycle_height(",
                ),
            )
            output_rollover = region(
                launch_path,
                launch_source,
                "typed lifecycle finalized-output rollover",
                "impl FinalizedProductionLifecycleRolloverV1",
                "impl ProductionLifecyclePostOutputHandoffV1",
            )
            require_order(
                launch_path,
                "typed lifecycle finalized-output rollover",
                output_rollover,
                (
                    "rollover_finalized_height_outputs_for_lifecycle(",
                    "ProductionLifecycleOutputRolloverPermitV1 {",
                    "finalized_adapter.retire_after_output_handoff()",
                    "refresh_live_serve_retirement_cut(&services, &retired_ingress)",
                    "ProductionLifecyclePostOutputHandoffV1 {",
                ),
            )
            require_tokens(
                finalized_output_path,
                "sealed runner finalized-output reuse",
                finalized_output_source,
                (
                    "fn rollover_finalized_height_outputs_for_lifecycle(",
                    "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1",
                    "rollover_finalized_height_outputs(",
                ),
            )
            store_retirement = region(
                launch_path,
                launch_source,
                "post-output lifecycle-store retirement",
                "impl ProductionLifecyclePostOutputHandoffV1",
                "impl ProductionLifecycleCleanupReadyV1",
            )
            require_order(
                launch_path,
                "post-output lifecycle-store retirement",
                store_retirement,
                (
                    "begin_fail_stop_operation()",
                    "retire_authenticated_cut(serve_payloads, &retained_serve_payloads)",
                    "reconcile_complete_tip_serve_retirement(&current, refreshed)",
                    "stage_finalized_height_all_row_retirement(reconciliation)",
                    "persist_exact_finalization_successor(staged)",
                    "publication.consume_owners(registry)",
                    "operation.complete()",
                    "ProductionLifecycleCleanupReadyV1 {",
                ),
            )
            cleanup_ready = region(
                launch_path,
                launch_source,
                "cleanup-ready lifecycle service teardown",
                "impl ProductionLifecycleCleanupReadyV1",
                "impl ProductionLifecycleOwnerV1",
            )
            require_order(
                launch_path,
                "cleanup-ready lifecycle service teardown",
                cleanup_ready,
                (
                    "self.services.allow_clean_shutdown()",
                    "self.services.finish_height(self.receipt, cleanup_timeout, supervisor)",
                    "ProductionLifecycleFinalizationOutcomeV1 {",
                ),
            )
            _successor_recovery_finalization_tail(
                ledger_path,
                ledger_source,
                lifecycle_startup_test_path,
                lifecycle_startup_test_source,
                registry_validate_path,
                registry_validate_source,
                registry_validate_impl_path,
                registry_validate_impl_source,
                registry_path,
                registry_source,
                wal_recovery_path,
                wal_recovery_source,
                scheduler_path,
                scheduler_source,
                region,
                require_order,
                require_tokens,
                reject_tokens,
                require_literal_count,
            )
        _check_successor_snapshot_authority(
            recovery_path,
            recovery_source,
            region,
            require_tokens,
            require_order,
        )
    return errors
