# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _successor_recovery_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind recovered-height storage, lifecycle, and ingress sources."""

    errors: list[str] = []

    def load(relative: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "production successor-refinement source",
        )

    def region(
        path: Path,
        source: str,
        label: str,
        start_marker: str,
        end_marker: str,
    ) -> str:
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

    def require_literal_count(
        path: Path,
        label: str,
        body: str,
        literal: str,
        expected: int,
    ) -> None:
        observed = mask_rust_comments(body).count(literal)
        if observed != expected:
            errors.append(
                f"{path}: {label} must contain exact production literal "
                f"{literal!r} exactly {expected} time(s); found {observed}"
            )

    def require_order(
        path: Path,
        label: str,
        body: str,
        markers: tuple[str, ...],
    ) -> None:
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

    def reject_tokens(
        path: Path,
        label: str,
        body: str,
        forbidden: tuple[str, ...],
    ) -> None:
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
                    "exactly_covers_recovered_ready_work(&owner.coordinator)",
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
                    "fn recovered_wal_sign_status_publication_is_exact_last_and_unwired()",
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
                        "launched.drive_completion_turn(runner, &mut lane_work)",
                        "ProductionCompletionDispatchV1::ApplyQueued",
                        "launched.drive_completion_turn(runner, &mut lane_work)",
                        "ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyApplied",
                        ".initialize_recovered_local_proposal(setup_runner)",
                        "let mut activated = launched .activate(Instant::now(), activation, local_proposal_state)",
                        "drop(auxiliary_hold)", "ProductionLifecycleIngressSelectionV1::CertifiedServeQueued",
                        "assert_eq!(leader_wire_ingress.len(), 0)", "ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted",
                        "!selected.restart_required()",
                        "let claimed_producer = activated .claim_producer_turn_for_local_proposal(&mut serve_runner)",
                        "let attempted_producer = claimed_producer .into_attempted(producer_turn_attempt_permit_for_test(&mut serve_runner))",
                        "activated .settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
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
        if (
            launch_source
            and turn_driver_source
            and kura_source
            and owner_source
            and worker_source
            and scheduler_source
            and registry_source
            and registry_validate_source
            and concrete_admission_source
            and lifecycle_projection_source
            and wal_recovery_source
            and selector_source
            and body_pipeline_source
            and replay_authority_source
            and runtime_source
            and effects_source
            and transport_source
            and lifecycle_open_source
            and runner_dependency_source
            and finalized_output_source
            and lifecycle_startup_test_source
            and state_source
            and snapshot_source
            and schema_source
            and apply_source
        ):
            cold_apply_startup = _require_rust_item(
                ledger_path,
                ledger_source,
                "open_recovered_decision_apply_startup",
                errors,
            )
            if cold_apply_startup is not None:
                require_order(
                    ledger_path,
                    "cold recovered Decision Apply startup lineage",
                    cold_apply_startup.source,
                    (
                        "let (ledger_store, predecessor) = LifecycleLedgerStoreV1::open(",
                        "let fetch_is_present = predecessor",
                        "projection.fetch().names_record(record)",
                        "let staged_predecessor = if fetch_is_present",
                        "predecessor.clone()",
                        ".stage_authenticated_wal_decision_fetch(projection.fetch())",
                        "staged_predecessor .stage_recovered_decision_apply(projection.as_ref())",
                        "LifecycleCoordinator::prepare_with_authenticated_successor_store_borrowed(",
                        "authority, ledger_store, predecessor, successor.clone()",
                    ),
                )

            payload_terminal = _require_rust_item(
                schema_path,
                schema_source,
                "matches_terminal",
                errors,
            )
            if payload_terminal is not None:
                require_order(
                    schema_path,
                    "payload-free recovered Decision Fetch terminal identity",
                    payload_terminal.source,
                    (
                        "LifecycleWorkClass::Fetch, Self::None,",
                        "TerminalOutcome::Advanced",
                        "TerminalOutcome::Cancelled",
                        "TerminalOutcome::Rejected(_)",
                        "TerminalOutcome::Failed(_)",
                        "(LifecycleWorkClass::Fetch, Self::None, _) => false",
                    ),
                )
            restart_reconciliation = _require_rust_item(
                lifecycle_open_path,
                lifecycle_open_source,
                "reconcile_restart_inner",
                errors,
            )
            if restart_reconciliation is not None:
                require_order(
                    lifecycle_open_path,
                    "payload-free recovered Decision Fetch continuation",
                    restart_reconciliation.source,
                    (
                        "metadata.continuation.successor_parts()",
                        "recovered_decision_body_continuation_is_exact(",
                        ".or_else(|| { signed_broadcast_continuation_is_exact(",
                        ".unwrap_or_else(||",
                        "durable_continuation_payload_is_exact(",
                        "!payload_and_replay_are_exact",
                    ),
                )
            require_tokens(
                ledger_path,
                "payload-free signed-Broadcast restart regression",
                ledger_source,
                (
                    "fn all_sign_broadcast_continuations_roundtrip_with_canonical_wire_shapes()",
                    "exact_timeout_sign_broadcast_fixture(",
                    "durable_continuation_successor_is_exact(",
                    "signed_broadcast_continuation_is_exact(",
                    "Some(false)",
                    "parent.replay_authority_is_exact(context())",
                    "child.replay_authority_is_exact(context())",
                    "coordinator.reconcile_restart(RecoverySnapshot::new(",
                    "Some(super::super::CoordinatorFault::RecoveryRejected)",
                    "coordinator.records.is_empty()",
                ),
            )
            require_tokens(
                replay_authority_path,
                "recovered Decision body continuation regression",
                replay_authority_source,
                (
                    "fn recovered_decision_body_continuation_is_exact(",
                    "parent_payload == DurablePayloadReference::None",
                    "child_payload == DurablePayloadReference::BodyFrame(body_frame.durable_reference())",
                    "body_source.locator == fetch_locator",
                    "body_source.tag == fetch_tag",
                    "body_source.certificate == &fetch_certificate",
                    "fn recovered_decision_body_lineage_is_stage_closed_and_predecessor_bound()",
                ),
            )

            canonical_fragment = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "canonical_json_fragment",
                errors,
            )
            if canonical_fragment is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot JSON fragment identity",
                    canonical_fragment.source,
                    (
                        "let value: json::Value = json::from_str(input)",
                        "json::to_json(&value)",
                    ),
                )
            scalar_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_snapshot_wsv_hash",
                errors,
            )
            if scalar_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot scalar identity",
                    scalar_hash.source,
                    (
                        "Some(_) =>",
                        "let canonical = canonical_json_fragment(input)?",
                        "Digest::update(hasher, canonical.as_bytes())",
                    ),
                )
            object_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_snapshot_wsv_object_hash",
                errors,
            )
            if object_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical staged snapshot event-buffer identity",
                    object_hash.source,
                    (
                        "path == CanonicalWsvPath::World",
                        "!members.iter().any(|member| member.key == \"external_event_buf\")",
                        "let Some(value) = overrides.committed_external_event_buf",
                        "members.push(BorrowedJsonMember",
                        "key: \"external_event_buf\".to_owned()",
                        "members.sort_unstable_by(",
                    ),
                )
                require_order(
                    snapshot_path,
                    "canonical snapshot object-key identity",
                    object_hash.source,
                    (
                        "let canonical_key = canonical_json_fragment(member.encoded_key)?",
                        "Digest::update(hasher, canonical_key.as_bytes())",
                    ),
                )
            string_set_hash = _require_rust_item(
                snapshot_path,
                snapshot_source,
                "update_sorted_string_set_hash",
                errors,
            )
            if string_set_hash is not None:
                require_order(
                    snapshot_path,
                    "canonical snapshot string-set identity",
                    string_set_hash.source,
                    (
                        "items.iter().any(|item| !item.starts_with('\"'))",
                        ".map(canonical_json_fragment)",
                        ".collect::<Result<Vec<_>, _>>()?",
                        "items.sort_unstable()",
                        "items.dedup()",
                    ),
                )
            require_tokens(
                snapshot_path,
                "canonical snapshot hash behavior",
                snapshot_source,
                (
                    "fn borrowed_snapshot_wsv_hash_canonicalizes_json_lexemes()",
                    "fn staged_snapshot_wsv_hash_injects_committed_event_buffer()",
                    "committed_external_event_buf: Some(committed_event_buffer)",
                    "Hash::new(canonical)",
                ),
            )
            apply_behavior = _require_rust_item(
                apply_path,
                apply_source,
                "validate_and_apply",
                errors,
            )
            if apply_behavior is not None:
                require_order(
                    apply_path,
                    "staged and committed snapshot hash parity",
                    apply_behavior.source,
                    (
                        "let staged_snapshot_bytes_for_test =",
                        "canonical_staged_state_snapshot_hash(&state_block)",
                        "staged_checkpoint, Hash::new(",
                        "store_wsv_checkpoint(context.height, block_hash, staged_checkpoint)",
                        "let committed = crate::snapshot::canonical_state_snapshot_bytes(self.state.as_ref())",
                        "crate::snapshot::canonical_state_snapshot_hash(self.state.as_ref())",
                        "Hash::new(&committed)",
                        "if staged != committed",
                    ),
                )

            all_live_census = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "exactly_covers_all_live_work_with_optional_active_producer",
                errors,
            )
            if all_live_census is not None:
                require_tokens(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    (
                        "coordinator.capacity_generation.keys().copied().collect::<std::collections::BTreeSet<_>>() != exact_capacity_classes",
                        "coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS",
                        "candidate.replay_authority_is_exact(coordinator.active_context)",
                        "waiting.wait_token.observed_generation > coordinator.capacity_generation[&class]",
                        "LifecycleLedgerV1::from_coordinator(coordinator)",
                        "coordinator.episode_authority.universe_for(record.key).as_ref() != Some(&record.episode.universe)",
                        "wait.observed_generation == u64::MAX",
                        "coordinator.observed_generation.get(&wait.source).copied().unwrap_or(0) != wait.observed_generation",
                        "coordinator.owner_index != exact_owners",
                        "coordinator.ready_index != exact_ready",
                        "coordinator.capacity_used != exact_capacity_used",
                        "self.entries.len() != live.len()",
                        "serve_ordinal_pair_is_exact(serve, producer)",
                        "Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)",
                        "!paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)",
                        "replay_authority == &metadata.replay_authority",
                        "sign.dispatch_key.is_none()",
                        "sign.repair.validates_in_ledger(&exact_ledger)",
                        "sign.carrier.validates_in_ledger(verified, &exact_ledger)",
                        "broadcast.validates_in_ledger(&exact_ledger)",
                        "fetch.carrier.validates_in_ledger(verified, &exact_ledger)",
                        "match (fetch.dispatch_key, fetch.wait_source)",
                        "(None, None)",
                        "(Some(key), Some(source))",
                        "key.matches(coordinator.active_context, address, digest)",
                        "fetch.matches_waiting_record( address, digest, coordinator, source, )",
                        "(None, Some(_)) | (Some(_), None) => false",
                        "store.fetch.validates(verified)",
                        "store.fetch.validates_recovered_store_in_ledger(&store.store, &exact_ledger)",
                        "apply.dispatch_key.is_none()",
                        "apply.carrier.validates_in_ledger(",
                        "verified, &exact_ledger, address.ordinal",
                    ),
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "coordinator.admission_waits.len() > super::MAX_PENDING_ADMISSION_WAITS",
                    1,
                )
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    (
                        "let exact_capacity_classes = CapacityClass::ALL",
                        "coordinator.admission_waits.iter().any",
                        "LifecycleLedgerV1::from_coordinator(coordinator)",
                        "coordinator.records.iter().any",
                        "coordinator.capacity_used != exact_capacity_used",
                        "let live = coordinator.records.iter()",
                        "self.entries.len() != live.len()",
                        "coordinator.producer_debts.iter().all",
                        "!paired_next_vote_addresses.is_subset(&exact_next_vote_addresses)",
                        "live.into_iter().all",
                    ),
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "sign.dispatch_key.is_none()",
                    3,
                )
                require_token_count(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    all_live_census.source,
                    "apply.dispatch_key.is_none()",
                    1,
                )
            fresh_serve_preflight = _require_rust_item(
                registry_path,
                registry_source,
                "preflights_fresh_registry",
                errors,
            )
            if fresh_serve_preflight is not None:
                require_order(
                    registry_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_preflight.source,
                    (
                        "self.preflights_registry(registry)",
                        "registry.exactly_covers_all_live_work(verified, current)",
                        "current.active_context == staged.active_context",
                        "self.exactly_matches_fresh_staged_append(current, staged)",
                    ),
                )
            fresh_serve_install = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "install_certified_serve_fresh_batch_before_publication",
                errors,
            )
            if fresh_serve_install is not None:
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_install.source,
                    (
                        "batch.preflights_fresh_registry(self, verified, current, staged)",
                        "return Err(CertifiedServeRegistryBatchPublicationError::Preflight(",
                        "batch",
                        "self.install_certified_serve_batch_before_publication(batch, publish)",
                    ),
                )
            fresh_serve_owner = _require_rust_item(
                lifecycle_projection_path,
                lifecycle_projection_source,
                "admit_selected_certified_serve",
                errors,
            )
            if fresh_serve_owner is not None:
                require_order(
                    lifecycle_projection_path,
                    "fresh Serve exhaustive all-live registry census",
                    fresh_serve_owner.source,
                    (
                        "!registry.exactly_covers_all_live_work(&self.verified, &self.coordinator)",
                        "retain_for_admission_with_verified_retention",
                        "let mut staged = self.coordinator.stage_durable_transaction()",
                        "PreparedCertifiedServeRegistryBatchV1::from_fresh_admitted_pair",
                        "install_certified_serve_fresh_batch_before_publication",
                        "self.coordinator.persist_exact_staged_successor(&staged)",
                        "self.coordinator = staged",
                    ),
                )
            require_tokens(
                ledger_operations_path,
                "fresh Serve exhaustive all-live registry census",
                ledger_operations_source,
                (
                    "fn exactly_matches_recovered_decision_apply_carrier(",
                    "installed_apply_ordinal: u128",
                    "!changed && staged == *self && apply_ordinal == installed_apply_ordinal",
                ),
            )
            require_tokens(
                concrete_admission_path,
                "fresh Serve exhaustive all-live registry census regressions",
                concrete_admission_source,
                (
                    "fn exhaustive_live_registry_census_rejects_volatile_drift_and_one_missing_carrier()",
                    "WaitSource::Capacity(super::super::CapacityClass::Consensus)",
                    "WaitSource::Recovery(LifecycleDigest::new([0x33; 32]))",
                    "coordinator.observed_generation.insert(recovery_source, 1)",
                    "WaitToken::new(recovery_source, u64::MAX)",
                    ".capacity_generation.remove(&super::super::CapacityClass::Producer)",
                    ".episode.frozen_predecessors.insert(1)",
                    "remove_exact_for_test(address)",
                ),
            )
            recovered_broadcast_pair_fixture = _require_rust_item(
                registry_validate_path,
                registry_validate_source,
                "recovered_broadcast_pair_scheduler_fixture_for_test",
                errors,
            )
            if recovered_broadcast_pair_fixture is not None:
                require_order(
                    registry_validate_path,
                    "fresh Serve exhaustive all-live registry census regressions",
                    recovered_broadcast_pair_fixture.source,
                    (
                        "paired_next_sign",
                        "unrelated_sign",
                        "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote",
                        "attest_ready_recovered_lifecycle_sign",
                        "exactly_covers_all_live_work(verified, coordinator)",
                    ),
                )
            require_tokens(
                ledger_path,
                "fresh Serve exhaustive all-live registry census regressions",
                ledger_source,
                (
                    "fn fresh_certified_serve_publishes_exact_ledger_beside_fetch_and_broadcast()",
                    "exactly_covers_all_live_work(&fixture.verified, &owner.coordinator)",
                    "owner.live_fetch_count_for_test()",
                ),
            )
            lifecycle_launch_item = _require_qualified_rust_item(
                launch_path,
                launch_source,
                "ProductionLifecycleOwnerV1",
                "launch",
                errors,
                "Kura-bound production lifecycle launch",
                expected_attributes=(
                    "#[allow(clippy::result_large_err)]",
                    "#[inline(never)]",
                ),
            )
            lifecycle_launch = (
                lifecycle_launch_item.source
                if lifecycle_launch_item is not None
                else ""
            )
            require_order(
                launch_path,
                "Kura-bound production lifecycle launch",
                lifecycle_launch,
                (
                    "begin_fail_stop_operation()",
                    "Self::launch_local_identity_matches( &context.roster, &inputs.local_peer, inputs.local_validator, &inputs.key_pair, )",
                    "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
                    "service.matches_lifecycle_launch( &inputs.state, &inputs.kura, &context, &validator_set_pops, )",
                    "binding.storage_paths_for_launch(inputs.kura.as_ref())",
                    "prepare_leader_wire_launch(launch_storage.wal_path())",
                    "RuntimeLifecycleOrdinalSource::after_high_watermark(0)",
                    "leader_wire_launch.restored_producer_ordinal_high_watermark()",
                    "leader_wire_launch.open_gate(",
                    "leader_wire_restore.scheduler_ordinal_high_watermark()",
                    "ProductionLeaderWireIngressBindingV1::bind(",
                    "self.adapter_startup.take()",
                    "self.body_store.take()",
                    "self.apply_service.take()",
                    "V2EffectExecutor::open_with_body_store(",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "executor.install_authenticated_genesis_body(authenticated_genesis.signed_block())",
                    "ProductionV2Services::start_with_apply_service(",
                    "ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service,",
                    "leader_wire_ingress_binding,",
                ),
            )
            runner_dependency_permit = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-sealed recovered lifecycle factory dependency permit",
                "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                "/// Runner-private one-shot authority for activating a launched lifecycle height.",
            )
            require_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                    "local_signer: KeyPair",
                    "block_cadence: Duration",
                "fn mint_for_recovered_runner(local_signer: KeyPair, block_cadence: Duration,) -> Self",
                    "#[cfg(test)] pub(in crate::sumeragi) fn for_test(local_signer: KeyPair, block_cadence: Duration) -> Self",
                    "fn into_factory_dependencies(self) -> (KeyPair, Duration)",
                    "(self.local_signer, self.block_cadence)",
                    "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
                    "pub(crate) fn mint_for_recovered_runner(",
                    "pub fn mint_for_recovered_runner(",
                    "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "fn into_parts(",
                ),
            )
            lifecycle_activation = region(
                launch_path,
                launch_source,
                "one-shot lifecycle activation transaction",
                "fn activate_with(",
                "\n}\n\nimpl ActivatedProductionLifecycleV1",
            )
            require_order(
                launch_path,
                "one-shot lifecycle activation transaction",
                lifecycle_activation,
                (
                    "begin_fail_stop_operation()",
                    "self.executor.local_proposal_directive()",
                    "local_proposal.exactly_matches( self.executor.context().id(), current_directive )",
                    "ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch",
                    "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
                    "self.executor.arm_live_clocks(clock_activation, now)",
                    "self.executor.successor_activation_status_snapshot()",
                    "self.completion_observer_activation.take()",
                    "self.services.activate_effect_completion_observer(observer)",
                    "publication.open_and_publish( &self.leader_wire_ingress_binding.ingress, status )?",
                    "activation.complete()",
                    "ActivatedProductionLifecycleV1 { runner_activation, local_proposal, launched: self, }",
                ),
            )
            reject_tokens(
                launch_path,
                "one-shot lifecycle activation transaction",
                lifecycle_activation,
                (
                    "set_v2_status",
                    "into_parts",
                    "into_owner",
                    "into_executor",
                    "into_services",
                ),
            )
            activated_owner = region(
                launch_path,
                launch_source,
                "opaque activated lifecycle owner",
                "struct ActivatedProductionLifecycleV1",
                "enum ProductionLifecycleActivationPublicationV1",
            )
            require_tokens(
                launch_path,
                "opaque activated lifecycle owner",
                activated_owner,
                (
                    "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1",
                    "local_proposal: ProductionLifecyclePreparedLocalProposalStateV1",
                    "launched: LaunchedProductionLifecycleV1",
                ),
            )
            require_order(
                launch_path,
                "opaque activated lifecycle owner drop order",
                activated_owner,
                (
                    "runner_activation: super::super::v2_runner::ProductionLifecycleActivatedRunnerAuthorityV1",
                    "local_proposal: ProductionLifecyclePreparedLocalProposalStateV1",
                    "launched: LaunchedProductionLifecycleV1",
                ),
            )
            reject_tokens(
                launch_path,
                "opaque activated lifecycle owner",
                activated_owner,
                (
                    "pub launched:",
                    "pub(crate) launched:",
                    "pub(in crate::sumeragi) launched:",
                    "pub runner_activation:",
                    "pub(crate) runner_activation:",
                    "pub(in crate::sumeragi) runner_activation:",
                    "pub local_proposal:",
                    "pub(crate) local_proposal:",
                    "pub(in crate::sumeragi) local_proposal:",
                    "impl Clone for ActivatedProductionLifecycleV1",
                    "impl Copy for ActivatedProductionLifecycleV1",
                ),
            )
            activated_runner_borrow = region(
                launch_path,
                launch_source,
                "borrow-bound activated lifecycle owner",
                "impl ActivatedProductionLifecycleV1",
                "impl ProductionLifecycleOwnerV1",
            )
            require_tokens(
                launch_path,
                "borrow-bound activated lifecycle owner",
                activated_runner_borrow,
                (
                    "fn with_runner_runtime<R>(",
                    "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
                    "&mut super::super::v2_runner::ProductionLifecycleLocalProposalStateV1",
                    ".prepared_local_proposal_mut()",
                    "&mut self.launched.owner",
                    "&mut self.launched.executor",
                    "&mut self.launched.services",
                    "local_proposal",
                ),
            )
            reject_tokens(
                launch_path,
                "borrow-bound activated lifecycle owner",
                activated_runner_borrow,
                (
                    "into_parts",
                    "into_owner",
                    "into_executor",
                    "into_services",
                    "pub launched:",
                    "pub(crate) launched:",
                ),
            )
            ordinary_runner_activation = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned lifecycle activation authority",
                "struct ProductionLifecycleRunnerActivationV1",
                "struct ProductionLifecycleCompleteTipRunnerActivationV1",
            )
            require_order(
                runner_dependency_path,
                "runner-owned lifecycle activation authority",
                ordinary_runner_activation,
                (
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
                    "self.block_ingress.close()",
                    "self.block_ingress.open()",
                    "let publication = match self.status",
                    "self.block_ingress.close()",
                    "self.ingress_ready.store(true, Ordering::Release)",
                ),
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned lifecycle activation status classes",
                ordinary_runner_activation,
                (
                    "_seal: ProductionLifecycleRunnerActivationSealV1",
                    "struct ProductionLifecycleRunnerActivationSealV1",
                    "impl Drop for ProductionLifecycleRunnerActivationSealV1",
                    "fn current_height(",
                    "fn applied(",
                    "fn snapshot_bootstrap(",
                    "status: ProductionLifecycleRunnerStatusAuthorityV1",
                    "CurrentHeight",
                    "Applied",
                    "SnapshotBootstrap",
                    "status::set_v2_status(successor)",
                    "status::activate_v2_successor_height(",
                    "status::activate_snapshot_bootstrap_v2_height(",
                    "ProductionLifecycleActivatedRunnerAuthorityV1 { _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1, ingress_ready: self.ingress_ready, block_ingress: self.block_ingress, }",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned lifecycle activation status classes",
                ordinary_runner_activation,
                (
                    "impl Clone for ProductionLifecycleRunnerActivationV1",
                    "impl Copy for ProductionLifecycleRunnerActivationV1",
                    "pub(in crate::sumeragi) fn current_height(",
                    "pub(crate) fn current_height(",
                    "pub fn current_height(",
                    "pub(in crate::sumeragi) fn applied(",
                    "pub(in crate::sumeragi) fn snapshot_bootstrap(",
                    "fn into_parts(",
                ),
            )
            complete_tip_runner_activation = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned CompleteTip lifecycle activation authority",
                "struct ProductionLifecycleCompleteTipRunnerActivationV1",
                "struct ProductionLifecycleActivatedRunnerAuthorityV1",
            )
            require_order(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation authority",
                complete_tip_runner_activation,
                (
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
                    "self.block_ingress.close()",
                    "retirement.authorizes_successor_status(&successor)",
                    "self.block_ingress.close()",
                    "self.block_ingress.open()",
                    "status::activate_recovered_complete_tip_v2_height(retirement, successor)",
                    "self.block_ingress.close()",
                    "self.ingress_ready.store(true, Ordering::Release)",
                ),
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation seal",
                complete_tip_runner_activation,
                (
                    "_seal: ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "struct ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "impl Drop for ProductionLifecycleCompleteTipRunnerActivationSealV1",
                    "fn mint_for_recovered_runner(",
                    "ProductionLifecycleActivatedRunnerAuthorityV1 { _seal: ProductionLifecycleActivatedRunnerAuthoritySealV1, ingress_ready: self.ingress_ready, block_ingress: self.block_ingress, }",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned CompleteTip lifecycle activation seal",
                complete_tip_runner_activation,
                (
                    "impl Clone for ProductionLifecycleCompleteTipRunnerActivationV1",
                    "impl Copy for ProductionLifecycleCompleteTipRunnerActivationV1",
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
                    "pub(crate) fn mint_for_recovered_runner(",
                    "pub fn mint_for_recovered_runner(",
                    "fn into_parts(",
                ),
            )
            activated_runner_authority = region(
                runner_dependency_path,
                runner_dependency_source,
                "activated runner readiness and ingress authority",
                "struct ProductionLifecycleActivatedRunnerAuthorityV1",
                "struct ProductionLifecycleActiveRunnerBorrowV1",
            )
            require_tokens(
                runner_dependency_path,
                "activated runner readiness and ingress authority",
                activated_runner_authority,
                (
                    "_seal: ProductionLifecycleActivatedRunnerAuthoritySealV1",
                    "ingress_ready: Arc<AtomicBool>",
                    "block_ingress: Arc<FairV2Ingress>",
                    "impl Drop for ProductionLifecycleActivatedRunnerAuthoritySealV1",
                    "fn retire(",
                    "self.ingress_ready.store(false, Ordering::Release)",
                    "self.block_ingress.close()",
                    "retire_lifecycle_runner_ingress(&self.ingress_ready, &self.block_ingress, launched_ingress)",
                    "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "activated runner readiness and ingress authority",
                activated_runner_authority,
                (
                    "impl Clone for ProductionLifecycleActivatedRunnerAuthorityV1",
                    "impl Copy for ProductionLifecycleActivatedRunnerAuthorityV1",
                    "fn into_parts(",
                    "pub ingress_ready:",
                    "pub block_ingress:",
                ),
            )
            require_token_count(
                runner_dependency_path,
                "activated runner readiness retirement",
                activated_runner_authority,
                "self.ingress_ready.store(false, Ordering::Release)",
                1,
            )
            require_token_count(
                runner_dependency_path,
                "activated runner ingress retirement",
                activated_runner_authority,
                "self.block_ingress.close()",
                1,
            )
            shared_runner_ingress_retirement = region(
                runner_dependency_path,
                runner_dependency_source,
                "shared lifecycle runner ingress retirement",
                "fn retire_lifecycle_runner_ingress(",
                "impl Drop for ProductionLifecycleActivatedRunnerAuthorityV1",
            )
            require_order(
                runner_dependency_path,
                "shared lifecycle runner ingress retirement",
                shared_runner_ingress_retirement,
                (
                    "ingress_ready.store(false, Ordering::Release)",
                    "block_ingress.close()",
                    "Arc::ptr_eq(block_ingress, launched_ingress)",
                ),
            )
            active_runner_borrow = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-owned active lifecycle borrow key",
                "struct ProductionLifecycleActiveRunnerBorrowV1",
                "/// Process-local borrow key for preparing a launched lifecycle before activation.",
            )
            require_tokens(
                runner_dependency_path,
                "runner-owned active lifecycle borrow key",
                active_runner_borrow,
                (
                    "_seal: ProductionLifecycleActiveRunnerBorrowSealV1",
                    "fn mint_for_recovered_runner() -> Self",
                    "impl Drop for ProductionLifecycleActiveRunnerBorrowSealV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-owned active lifecycle borrow key",
                active_runner_borrow,
                (
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner",
                    "pub(crate) fn mint_for_recovered_runner",
                    "pub fn mint_for_recovered_runner",
                    "fn into_parts(",
                    "impl Clone for ProductionLifecycleActiveRunnerBorrowV1",
                ),
            )
            require_tokens(
                launch_path,
                "local launch identity preflight",
                launch_source,
                (
                    "fn launch_local_identity_matches(",
                    "local_peer.public_key() != key_pair.public_key()",
                    "local_validator.is_none_or(|observed| roster_position == Some(observed))",
                    "fn launch_local_identity_requires_the_bound_key_and_exact_roster_position()",
                ),
            )
            require_tokens(
                launch_path, "single restored lifecycle ordinal source", lifecycle_launch,
                (
                    "inputs.network.reply_route_source_capacity().max(1)", "inputs.auxiliary_io_capacity",
                    "lifecycle_ordinals.clone()", "lifecycle_ordinals .advance_past(leader_wire_restore.scheduler_ordinal_high_watermark())",
                ),
            )
            require_token_count(
                launch_path, "single restored lifecycle ordinal source",
                lifecycle_launch, "lifecycle_ordinals.clone()", 2,
            )
            require_token_count(
                launch_path,
                "certified Serve restore/start capacity parity",
                lifecycle_launch,
                "inputs.auxiliary_io_capacity",
                2,
            )
            require_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                launch_source,
                (
                    "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "authenticated_genesis.signed_block()",
                ),
            )
            reject_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                region(
                    launch_path,
                    launch_source,
                    "sealed production launch inputs",
                    "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
                    "\n}",
                ),
                (
                    "authenticated_genesis: Option<SignedBlock>",
                    "genesis_account: AccountId",
                    "chunk_root: PathBuf",
                    "wal_path: PathBuf",
                    "lifecycle_ordinals: RuntimeLifecycleOrdinalSource",
                    "durable_bodies:",
                    "recovered_body_receipts:",
                    "queue: Arc<Queue>",
                    "provider_ingest_finalized_archive:",
                    "reputation_finalized_archive:",
                    "block_cadence: Duration",
                    "events_sender: EventsSender",
                ),
            )
            require_tokens(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                (
                    "fn start_with_apply_service(",
                    "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
                    "Self::start_inner(",
                ),
            )
            legacy_worker_start = region(
                worker_path,
                worker_source,
                "legacy worker Apply-service construction",
                "pub(crate) fn start(",
                "pub(in crate::sumeragi) fn start_with_apply_service(",
            )
            require_order(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                (
                    "let apply_service = V2ApplyService::new(",
                    "Self::start_inner(",
                ),
            )
            reject_tokens(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                ("Self::start_with_apply_service(",),
            )
            require_token_count(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1",
                1,
            )
            require_token_count(
                launch_path,
                "sealed replay-service permit mint",
                launch_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1 {",
                1,
            )
            require_tokens(
                state_path,
                "fixed State/Kura identity oracle",
                state_source,
                (
                    "fn matches_kura_instance(&self, kura: &Arc<Kura>) -> bool",
                    "Arc::ptr_eq(&self.kura, kura)",
                ),
            )
            require_tokens(
                apply_path,
                "fixed recovered Apply-service identity oracle",
                apply_source,
                (
                    "fn matches_lifecycle_launch(",
                    "Arc::ptr_eq(&self.state, state)",
                    "Arc::ptr_eq(&self.kura, kura)",
                    "self.network_id == context.network_id",
                    "self.validator_set_pops == validator_set_pops",
                ),
            )
            require_tokens(
                launch_path,
                "sealed leader-wire launch binding",
                launch_source,
                (
                    "struct ProductionLeaderWireIngressBindingV1",
                    "gate: Option<Arc<LeaderWireLifecycleStoreGate>>",
                    "fn bind(",
                    "ingress.bind_leader_wire_lifecycle_gate(",
                    "fn retire(&mut self)",
                    "self.gate.as_ref().cloned()",
                    "self.ingress.retire_leader_wire_lifecycle_gate(&gate)",
                    "self.gate = None",
                    "impl Drop for ProductionLeaderWireIngressBindingV1",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            require_tokens(
                adapter_path,
                "sealed adapter leader-wire launch projection",
                adapter_source,
                (
                    "struct ProductionLeaderWireLaunchAuthorityV1",
                    "fn prepare_leader_wire_launch(",
                    "adapter.wal.matches_path(expected_wal_path)",
                    "leader_wire_launch_prepared: false",
                    "!*leader_wire_launch_prepared",
                    "*leader_wire_launch_prepared = true",
                    "fn open_gate(",
                    "body_store: &super::v2_body_store::V2BodyStore",
                    "body_store.matches_context(context)",
                    "body_store.recovery_catalog()",
                    "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(",
                ),
            )
            require_tokens(
                safety_wal_path,
                "opened safety-WAL directory authority",
                safety_wal_source,
                (
                    "struct SafetyWalServicedCandidateStoreAuthority",
                    "struct SafetyWalLeaderWireStoreAuthority",
                    "direct_lexical_directory_metadata(expected_path)",
                    "open_canonical_directory_nofollow(&canonical_path)",
                    "fn mint_serviced_candidate_store_authority(",
                    "fn mint_leader_wire_store_authority(",
                    "fn publish_atomic(&self, frame: &[u8], maximum: u64",
                    "let durable = rustix::fs::statat(",
                    "fn write_all(&mut self, bytes: &[u8])",
                    "fn sync_data(&mut self)",
                    "BoundSafetyWalDirectory::from_kura_authority(kura, authority)",
                ),
            )
            require_literal_count(
                safety_wal_path,
                "opened safety-WAL exact Kura identity rejection",
                safety_wal_source,
                '"safety-WAL authority belongs to a different Kura instance"',
                1,
            )
            require_tokens(
                kura_path,
                "Kura-root safety-WAL authority",
                kura_source,
                (
                    "struct KuraSafetyWalDirectoryAuthority",
                    "fn mint_safety_wal_directory_authority(",
                    "rustix::fs::openat(&root.file, STORE_ROOT_LOCK_FILE_NAME",
                    "Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)",
                    "rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU)",
                    "Self::open_bound_progress_child_directory(",
                    "kura_identity: self.instance_identity()",
                ),
            )
            reject_tokens(
                safety_wal_path,
                "move-only safety-WAL sibling authorities",
                safety_wal_source,
                (
                    "impl Clone for SafetyWalServicedCandidateStoreAuthority",
                    "impl Clone for SafetyWalLeaderWireStoreAuthority",
                    "impl Copy for SafetyWalServicedCandidateStoreAuthority",
                    "impl Copy for SafetyWalLeaderWireStoreAuthority",
                ),
            )
            require_tokens(
                adjacent_store_path,
                "typed WAL-adjacent production stores",
                adjacent_store_source,
                (
                    "storage: SafetyWalServicedCandidateStoreAuthority",
                    "storage: SafetyWalLeaderWireStoreAuthority",
                    "fn open_with_safety_wal_authority(",
                    "self.storage.read_bounded(self.max_frame_bytes)",
                    "self.storage.publish_atomic(&frame, self.max_frame_bytes)",
                ),
            )
            serviced_candidate_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "ServicedCandidateStore",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                serviced_candidate_open,
                "storage: SafetyWalServicedCandidateStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
                errors,
            )
            leader_wire_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "LeaderWireLifecycleStoreGate",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                expected_attributes=("#[allow(clippy::too_many_arguments)]",),
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                leader_wire_open,
                "storage: SafetyWalLeaderWireStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                errors,
            )
            reject_tokens(
                adapter_path,
                "move-only leader-wire launch authority",
                adapter_source,
                (
                    "impl Clone for ProductionLeaderWireLaunchAuthorityV1",
                    "impl Clone for RecoveredLifecycleStorageAuthorityV1",
                    "impl Clone for RecoveredLifecycleLaunchStoragePathsV1",
                ),
            )
            require_tokens(
                owner_path,
                "production lifecycle owner Kura seal",
                owner_source,
                (
                    "kura_binding: Option<crate::sumeragi::v2::RecoveredLifecycleOwnerKuraBindingV1>",
                    "apply_service: Option<crate::sumeragi::v2_apply::V2ApplyService>",
                    "fn with_recovered_kura_binding_and_apply_service(",
                    "assert!(self.kura_binding.is_none())",
                    "assert!(self.apply_service.is_none())",
                    "self.kura_binding = Some(binding)",
                    "self.apply_service = Some(apply_service)",
                    "struct ProductionLifecycleApplyServiceLaunchPermitV1",
                    "impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1",
                ),
            )
            recovered_sign_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Sign dispatch",
                "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
                "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "let Some(body_store_identity) = self.body_store_identity.as_ref()",
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "attest_ready_recovered_lifecycle_sign",
                    "capture_recovered_lifecycle_sign_capacity(dispatch_key)",
                    "self.coordinator.plan_turn(inputs)",
                    "reservation.class() == CapacityClass::Consensus",
                    "prepare_recovered_lifecycle_sign_dispatch",
                    "reservation.preflight(&prepared)",
                    "reservation.commit(prepared)",
                ),
            )
            require_token_count(
                scheduler_path,
                "recovered Sign post-claim rollback",
                recovered_sign_dispatch,
                "self.coordinator.rollback_unpublished_turn(&lease)",
                1,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reserved post-claim rollback",
                recovered_sign_dispatch,
                "rollback_unpublished_reserved_turn(&lease",
                3,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reservation release",
                recovered_sign_dispatch,
                "reservation.cancel_uncommitted()",
                6,
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "AdapterEffect",
                    "PendingRuntimeEffectBinding",
                    "RuntimeEffectOwnership",
                    "EffectWorkId",
                    "into_parts",
                ),
            )
            recovered_phase_sign = region(
                registry_path,
                registry_source,
                "current-parent-bound recovered PhaseVote carrier",
                "impl DurableRecoveredWalSignWork {",
                "/// Whether one concrete registry row is still an executable adapter effect",
            )
            require_token_count(
                registry_path,
                "current-parent-bound recovered PhaseVote carrier",
                recovered_phase_sign,
                "self.matches_current_terminal_parent(coordinator)",
                2,
            )
            require_token_count(
                registry_path,
                "standalone recovered PhaseVote child",
                recovered_phase_sign,
                "metadata.continuation == super::schema::DurableContinuation::None",
                2,
            )
            require_tokens(
                registry_path,
                "current terminal Validate parent rejoin",
                recovered_phase_sign,
                (
                    "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
                    "metadata.matches_admission(parent)",
                    "super::schema::DurableContinuation::successor(",
                    "coordinator.key_index.get(&parent.key)",
                    "coordinator.owner_index.get(&parent.causal_root)",
                ),
            )
            recovered_sign_identity = region(
                registry_path,
                registry_source,
                "complete recovered Sign effect identity",
                "impl RecoveredLifecycleSignDispatchIdentityV1 {",
                "/// Read-only coordinates of one exact Waiting Fetch incumbent.",
            )
            require_tokens(
                registry_path,
                "complete recovered Sign effect identity",
                recovered_sign_identity,
                (
                    "&AdapterEffect::Sign {",
                    "request: request.clone()",
                    "adapter_effect_matches_lifecycle_digest(",
                ),
            )
            reject_tokens(
                registry_path,
                "historical recovered Commit identity",
                recovered_sign_identity,
                ("tag.view() ==", "vote.round.view"),
            )
            recovered_sign_task = region(
                worker_path,
                worker_source,
                "opaque recovered Sign worker task/result",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {",
                "enum V2IoCommand {",
            )
            require_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "identity: RecoveredLifecycleSignDispatchIdentityV1",
                    "prepared_candidate: Option<PreparedCandidateBody>",
                    "self.task.prepared_candidate == expected_prepared",
                    "outbound_payload: Option<EncodedV2Payload>",
                    "authorizes_request(self.task.tag, &self.task.request)",
                ),
            )
            reject_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "pub tag:",
                    "pub request:",
                    "pub signature:",
                    "pub outbound_payload:",
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                ),
            )
            parked_sign_completion = region(
                worker_path,
                worker_source,
                "parked recovered Sign completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {",
                "/// Result of atomically returning one guarded missing-sidecar Apply",
            )
            reject_tokens(
                worker_path,
                "parked recovered Sign completion",
                parked_sign_completion,
                (
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                    "fn settle(",
                ),
            )
            require_tokens(
                worker_path,
                "adapter-private recovered Sign completion projection",
                parked_sign_completion,
                (
                    "fn project_adapter_completion_authority(",
                    "result.is_exact()",
                    "RecoveredLifecycleSignAdapterCompletionAuthorityV1 {",
                ),
            )
            require_tokens(
                worker_path,
                "post-publication recovered Sign completion acknowledgement",
                parked_sign_completion,
                (
                    "fn acknowledge_after_publication(self)",
                    "self.queue.acknowledge_recovered_lifecycle_sign(key)",
                    "self.guarded.acknowledge_after_publication()",
                ),
            )
            recovered_sign_preview = region(
                adapter_path,
                adapter_source,
                "drop-inert recovered Sign adapter preview",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
                "/// Acknowledge successful application of the exact tagged decision.",
            )
            require_order(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "authority.consume_for_adapter(RecoveredLifecycleSignAdapterCompletionPermitV1::new())",
                    "verify_individual_signature(",
                    "let mut next_reducer = self.reducer.clone()",
                    "next_reducer.step(event.clone())",
                    "if converted.first() != Some(&expected_broadcast)",
                    "Ok(PreparedRecoveredLifecycleSignAdapterCompletionV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "closed recovered Sign adapter successor shapes",
                recovered_sign_preview,
                (
                    "SignRequest::Proposal(_), Some((persist_tag, entry)), None",
                    "SignRequest::Proposal(_), None, Some(AdapterEffect::Sign { request: SignRequest::Vote(vote), .. })",
                    "vote.phase == wire::GlobalPhase::Prepare",
                    "SignRequest::Vote(_) | SignRequest::TimeoutVote(_), None, possible_next_sign",
                    "next_reducer.pending_persistence_record().is_none()",
                    "next_reducer.awaiting_signature()",
                    "RecoveredLifecycleSignCompletionMismatch",
                ),
            )
            reject_tokens(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "self.wal.append(",
                    "self.reducer =",
                    "self.registry =",
                    "publish_effect",
                    "send(",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Sign adapter preview behavior regression",
                adapter_source,
                (
                    "fn recovered_timeout_signature_preview_is_exact_and_drop_inert()",
                    "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
                    "output.prepare_wal_append_permit().is_none()",
                ),
            )
            next_vote_service_join = region(
                worker_path,
                worker_source,
                "single-preview recovered next-Vote body service join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(",
                "pub(in crate::sumeragi) fn activate_effect_completion_observer(",
            )
            require_order(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "self.recovered_lifecycle_next_vote_body_executor_permit(executor)?",
                    "executor.prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)",
                ),
            )
            reject_tokens(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "ValidatedBodyReceipt",
                    "V2BodyStore",
                    "prepare_recovered_lifecycle_sign_completion(completion)",
                    "into_parts",
                ),
            )
            next_vote_executor_join = region(
                effects_path,
                effects_source,
                "single-preview recovered next-Vote body executor join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body(",
                "/// Publish executor-retained owners",
            )
            require_order(
                effects_path,
                "single-preview recovered next-Vote body executor join",
                next_vote_executor_join,
                (
                    "service.consume_for_executor(",
                    "runtime.prepare_recovered_lifecycle_sign_completion(completion)",
                    "preview.project_broadcast_and_sign_body_lookup(",
                    "preview.prepare_proposal_prepare_wal_body_lookup(",
                    "authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                    "Ok((preview, body))",
                ),
            )
            next_vote_catalog_join = region(
                effects_path,
                effects_source,
                "exact recovered next-Vote body catalog join",
                "fn authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                "impl V2EffectExecutor<SerializedV2Runtime>",
            )
            require_tokens(
                effects_path,
                "exact recovered next-Vote body catalog join",
                next_vote_catalog_join,
                (
                    "validated_bodies.get(&key) != Some(&validated)",
                    "durable_bodies.get(&key) != Some(durable)",
                    "recovered_bodies.get(&key)",
                    "HashOf::new(manifest) != durable.manifest_hash()",
                    "lookup.matches_recovered_body(manifest, recovered_durable)",
                    "RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1::new()",
                ),
            )
            next_vote_body_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered next-Vote body authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyAuthorityV1 {",
                "/// Closed reducer successor shape produced by one exact recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "body_store_identity.same_instance(expected_body_store_identity)",
                    "lookup.matches_adapter_successor(next_sign, expected_proposal_manifest_hash)",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "impl Clone for RecoveredLifecycleNextVoteBodyAuthorityV1",
                    "fn into_parts(",
                    "fn validated(",
                    "fn body_store_identity(",
                    "fn lookup(",
                ),
            )
            combined_adapter_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                "pub(in crate::sumeragi) fn project_broadcast_and_sign_authority(",
                "/// Exercise fail-closed next-Sign substitution",
            )
            require_order(
                adapter_path,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                combined_adapter_projection,
                (
                    "self.combined_authority_minted",
                    "body_authority.consume_for_adapter(",
                    "self.persisted_prepare_wal.is_some()",
                    "core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer)",
                    "core::mem::swap(&mut self.adapter.registry, &mut self.next_registry)",
                    "self.adapter.authenticate_recovered_lifecycle_next_vote(",
                    "core::mem::swap(&mut self.adapter.registry, &mut self.next_registry)",
                    "core::mem::swap(&mut self.adapter.reducer, &mut self.next_reducer)",
                    "self.combined_authority_minted = true",
                    "RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {",
                ),
            )
            proposal_output_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Proposal exact-output authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                "/// Adapter-authenticated combined successor of one recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "body_store_identity: V2BodyStoreInstanceIdentity",
                    "output_guard: Arc<super::output_guard::ConsensusOutputGuard>",
                    "fn consume_for_service(",
                    "fn from_service_retry(",
                    "Self::validated(",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "impl Clone for RecoveredLifecycleProposalExactOutputAuthorityV1",
                    "fn into_parts(",
                    "fn proposal(",
                    "fn payload(",
                    "fn body_store_identity(",
                    "fn output_guard(",
                ),
            )
            proposal_output_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Proposal exact-output projection",
                "pub(in crate::sumeragi) fn project_proposal_exact_output_authority(",
                "fn broadcast_proposal_manifest_hash(",
            )
            require_order(
                adapter_path,
                "affine recovered Proposal exact-output projection",
                proposal_output_projection,
                (
                    "let shape = self.shape()",
                    "self.proposal_output_authority_minted",
                    "!matches!( shape, RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal )",
                    "shape == RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "self.prepared_prepare_wal.is_none()",
                    "payload.manifest() == &signed.manifest",
                    "self.next_vote_body_store_identity.as_ref()",
                    "self.next_vote_output_guard.as_ref()",
                    "self.proposal_output_authority_minted = true",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "affine recovered Proposal exact-output projection",
                proposal_output_projection,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                ),
            )
            proposal_prepare_wal_preflight = region(
                adapter_path,
                adapter_source,
                "pre-WAL initial Proposal continuation",
                "pub(in crate::sumeragi) fn prepare_proposal_prepare_wal_body_lookup(",
                "/// Append and fsync the preflighted initial Proposal `PrepareIntent`.",
            )
            require_order(
                adapter_path,
                "pre-WAL initial Proposal continuation",
                proposal_prepare_wal_preflight,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "self.pending_prepare.as_ref().cloned()",
                    "expected_wal_sequence.checked_add(1) != Some(entry.id().get())",
                    "encode_wal_entry(&entry, self.adapter.aggregator.as_ref())",
                    "next_reducer.step(persisted_event.clone())",
                    "message: reducer::SignableMessage::Vote(vote)",
                    "RecoveredLifecycleNextVoteBodyLookupV1::from_adapter_preview(",
                    "self.next_vote_body_store_identity = Some(body_store_identity)",
                    "self.prepared_prepare_wal = Some(PreparedRecoveredLifecycleProposalPrepareWalV1 {",
                ),
            )
            reject_tokens(
                adapter_path,
                "mutation-free initial Proposal WAL preflight",
                proposal_prepare_wal_preflight,
                (".wal.append(", "self.adapter.reducer =", "self.adapter.registry ="),
            )
            proposal_prepare_wal_append = region(
                adapter_path,
                adapter_source,
                "fail-stop initial Proposal WAL append",
                "pub(in crate::sumeragi) fn append_recovered_lifecycle_proposal_prepare_wal(",
                "/// Project an inert exact-body lookup for the reducer-produced next Vote.",
            )
            require_order(
                adapter_path,
                "fail-stop initial Proposal WAL append",
                proposal_prepare_wal_append,
                (
                    "self.proposal_output_authority_minted",
                    "self.next_vote_body_store_identity.is_none()",
                    "self.next_vote_output_guard.is_none()",
                    "permit.authorizes(",
                    "self.adapter.pending_persistence_id = Some(persistence_id)",
                    "permit.cross_wal_attempt_boundary()",
                    "self.adapter.wal.append(&encoded_wal_payload)",
                    "LiveWalFrameIdentity::from_append_receipt(frame, receipt, persistence_id)",
                    "PendingRuntimeEffectBinding::from_exact_live_wal_append(",
                    "SealedLiveWalPersistedEffectV1::from_exact_live_append(",
                    "self.next_reducer = next_reducer",
                    "self.next_sign = Some(sign_effect)",
                    "self.pending_prepare = None",
                    "self.persisted_prepare_wal = Some(RecoveredLifecycleProposalPrepareWalContinuationV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "initial Proposal WAL ambiguity closes the adapter",
                proposal_prepare_wal_append,
                ("self.adapter.fail_closed = true", "WalFrameIdentityMismatch"),
            )
            proposal_batch_preflight = region(
                worker_path,
                worker_source,
                "mutation-free atomic Proposal fanout preflight",
                "fn prepare_atomic_fanout_batch(",
                "/// Commit a batch prepared while this exact mutex guard remained held.",
            )
            require_order(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "let mut additions = BTreeMap",
                    "aggregate.checked_add(count)",
                    "self.ownership_capacity_available(&additions)?",
                    "self.ownership_state_after_additions(&additions)?",
                    "let project_ids = |first: ExactFanoutFifoId|",
                    "self.source_fifo_owners.clone()",
                    "Some(existing_ids)",
                    "source_fifo_owners.entry(source).or_default().insert(fifo_id)",
                    "PendingExactOutputBatchPlan {",
                ),
            )
            reject_tokens(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "self.fanouts.extend(",
                    "self.source_fifo_owners =",
                    "self.reservation_owner_counts =",
                    "self.ownership_units =",
                    "rebase_source_fifo(",
                    "allocate_fanout_fifo_id(",
                    ".enqueue(",
                    "next_fanout_index =",
                ),
            )
            proposal_batch_commit = region(
                worker_path,
                worker_source,
                "assertion-only atomic Proposal fanout commit",
                "fn commit_atomic_fanout_batch(&mut self, plan: PendingExactOutputBatchPlan)",
                "fn is_pending(&self)",
            )
            require_order(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                (
                    "assert_eq!(self.fanouts.len(), existing_fanout_count",
                    "if let Some(rebased) = rebased_existing_fifo_ids",
                    "fanout.fifo_id = Some(fifo_id)",
                    "self.fanouts.extend(fanouts)",
                    "self.source_fifo_owners = source_fifo_owners",
                    "self.reservation_owner_counts = reservation_owner_counts",
                    "self.ownership_units = ownership_units",
                    "self.shared_ownership_units = shared_ownership_units",
                    "self.next_fanout_fifo_id = next_fanout_fifo_id",
                ),
            )
            reject_tokens(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                ("?", "drive_pending_exact_output", ".enqueue("),
            )
            proposal_reservation_fields = region(
                worker_path,
                worker_source,
                "fail-stop-first recovered Proposal reservation ownership",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputReservationV1<'service> {",
                "#[cfg_attr(not(test), allow(dead_code))]\nimpl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
            )
            require_order(
                worker_path,
                "fail-stop-first recovered Proposal reservation ownership",
                proposal_reservation_fields,
                (
                    "operation: Option<ConsensusFailStopOperation<'service>>",
                    "pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>",
                    "batch: Option<PendingExactOutputBatchPlan>",
                    "authority: Option<super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1>",
                    "wal_append: RecoveredLifecycleProposalPrepareWalAppendSealV1",
                ),
            )
            proposal_wal_append_seal = region(
                worker_path,
                worker_source,
                "reservation-bound initial Proposal WAL append authority",
                "struct RecoveredLifecycleProposalPrepareWalAppendSealV1 {",
                "#[cfg_attr(not(test), allow(dead_code))]\nimpl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
            )
            require_order(
                worker_path,
                "reservation-bound initial Proposal WAL append authority",
                proposal_wal_append_seal,
                (
                    "dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1",
                    "body_store_identity: V2BodyStoreInstanceIdentity",
                    "output_guard: Arc<ConsensusOutputGuard>",
                    "attempted: bool",
                    "pub(in crate::sumeragi) struct RecoveredLifecycleProposalPrepareWalAppendPermitV1<'reservation>",
                    "seal: &'reservation mut RecoveredLifecycleProposalPrepareWalAppendSealV1",
                    "!self.seal.attempted",
                    "self.seal.dispatch_key == dispatch_key",
                    ".same_instance(body_store_identity)",
                    "Arc::ptr_eq(&self.seal.output_guard, output_guard)",
                    "pub(in crate::sumeragi) fn cross_wal_attempt_boundary(self)",
                    "self.seal.attempted = true",
                ),
            )
            proposal_reservation_impl = region(
                worker_path,
                worker_source,
                "sealed recovered Proposal reservation methods",
                "impl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
                "pub(in crate::sumeragi) struct RecoveredDecisionFetchExactOutputReservationV1<'service> {",
            )
            require_order(
                worker_path,
                "armed Proposal reservation lends WAL authority without parts",
                proposal_reservation_impl,
                (
                    "pub(in crate::sumeragi) fn prepare_wal_append_permit(",
                    "self.operation.is_some()\n            && self.pending.is_some()\n            && self.batch.is_some()\n            && self.authority.is_some()\n            && !self.wal_append.attempted",
                    "seal: &mut self.wal_append",
                ),
            )
            proposal_reservation_abort = region(
                worker_path,
                proposal_reservation_impl,
                "retry-safe recovered Proposal reservation abort",
                "pub(in crate::sumeragi) fn abort_before_publication(",
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal reservation abort",
                proposal_reservation_abort,
                (
                    "assert!(\n            !self.wal_append.attempted",
                    "drop(self.pending.take())",
                    "drop(self.batch.take())",
                    ".complete()",
                    "self.authority.take()",
                ),
            )
            proposal_reservation_commit = proposal_reservation_impl.split(
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
                1,
            )[-1]
            require_order(
                worker_path,
                "assertion-only recovered Proposal reservation commit",
                proposal_reservation_commit,
                (
                    "let mut pending = self.pending.take()",
                    "let operation = self.operation.take()",
                    "let batch = self.batch.take()",
                    "let authority = self.authority.take()",
                    "pending.commit_atomic_fanout_batch(batch)",
                    "drop(pending)",
                    "drop(authority)",
                    "operation.complete()",
                ),
            )
            reject_tokens(
                worker_path,
                "sealed recovered Proposal reservation methods",
                proposal_reservation_abort + proposal_reservation_commit,
                ("drive_pending_exact_output", ".enqueue("),
            )
            proposal_output_capture = region(
                worker_path,
                worker_source,
                "retry-safe recovered Proposal exact-output capture",
                "pub(in crate::sumeragi) fn capture_recovered_lifecycle_proposal_exact_output(",
                "/// Consume one carrier-derived recovered Fetch through this exact service key.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal exact-output capture",
                proposal_output_capture,
                (
                    "self.proposal_work_retired",
                    "authority.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "tag != self.active_tag",
                    "self.local_validator != Some(proposal.proposer)",
                    "proposal.manifest != *payload.manifest()",
                    "identity.same_instance(&body_store_identity)",
                    "Arc::ptr_eq(&self.output_guard, &authority_output_guard)",
                    "message.validate_version()",
                    "proposal.validate(&self.context)",
                    "let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {",
                    "body_store_identity: body_store_identity.clone()",
                    "output_guard: Arc::clone(&authority_output_guard)",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1::from_service_retry(",
                    "payload.into_parts()",
                    "manifest.validate(&self.context)",
                    "chunk.signature_preimage(&self.context, &manifest)",
                    "Signature::try_new(self.key_pair.private_key(), &preimage)",
                    "let peers = self.remote_voters()",
                    "let control = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope())",
                    "let chunks = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "control.into_iter().chain(chunks)",
                    "begin_fail_stop_operation()",
                    "let pending = self.lock_pending_exact_output()?",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(retry_authority,)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(",
                    "authority: Some(retry_authority)",
                    "wal_append",
                ),
            )
            require_token_count(
                worker_path,
                "fail-stop recovered Proposal capture errors",
                proposal_output_capture,
                "drop(operation)",
                2,
            )
            reject_tokens(
                worker_path,
                "all-voter recovered Proposal retransmission policy",
                proposal_output_capture,
                ("fast_path_proposals", "remote_voters_for_indices"),
            )
            broadcast_consensus = region(
                worker_path,
                worker_source,
                "production consensus broadcast",
                "fn broadcast_consensus(",
                "fn sign_body_request(",
            )
            proposal_live_atomic = region(
                worker_path,
                broadcast_consensus,
                "live Proposal control-plus-chunk atomic transfer",
                "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {",
                "let control = vec![Self::preencode_v2_network_message(message)?]",
            )
            require_order(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "self.outbound_chunks.get(&manifest_hash)",
                    "let first_fast_path_send = !self.fast_path_proposals.contains(&proposal.round)",
                    "PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "self.enqueue_atomic_fanout_batch_while_guarded(",
                    "ownership == ExactFanoutOwnership::Owned && first_fast_path_send",
                    "self.fast_path_proposals.insert(proposal.round)",
                ),
            )
            reject_tokens(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "enqueue_exact_fanout_while_guarded(",
                    "self.fast_path_proposals.insert(proposal.round);\n            let payload_targets",
                ),
            )
            require_tokens(
                worker_path,
                "atomic Proposal output behavior regressions",
                worker_source,
                (
                    "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                    "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
                    "fn armed_recovered_proposal_output_reservation_fails_stop_on_drop()",
                    "fn proposal_broadcast_reports_source_retained_until_corridor_acceptance()",
                ),
            )
            proposal_output_behavior = region(
                worker_path,
                worker_source,
                "recovered Proposal atomic output behavior",
                "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                "fn prepare_and_commit_votes_reach_every_remote_voter_across_views()",
            )
            require_tokens(
                worker_path,
                "recovered Proposal atomic output behavior",
                proposal_output_behavior,
                (
                    "after, before",
                    "vec![Some(expected_batch_first_fifo), expected_batch_first_fifo.checked_add(1),]",
                    "fanout.peers.iter().cloned().collect::<BTreeSet<_>>()",
                    "wire::ConsensusMessageV2Payload::PayloadChunk(chunk)",
                    "chunk.validate(&service.context, manifest)",
                    "Signature::try_from_bytes(&chunk.signature)",
                    "signature.verify(signer.public_key()",
                    "capture_recovered_lifecycle_proposal_exact_output(retirement_authority).is_err()",
                ),
            )
            require_order(
                worker_path,
                "post-Decision live Proposal output fence",
                broadcast_consensus,
                (
                    "self.proposal_work_retired",
                    "wire::ConsensusMessageV2Payload::Proposal(_)",
                    "begin_fail_stop_operation()",
                    "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload",
                ),
            )
            next_vote_candidate_projection = region(
                replay_authority_path,
                replay_authority_source,
                "full executable recovered next-WAL-Vote candidate",
                "pub(in crate::sumeragi) fn into_candidate_projection(",
                "/// Rejoin the retained body marker to one exact recovered phase-vote repair.",
            )
            require_order(
                replay_authority_path,
                "full executable recovered next-WAL-Vote candidate",
                next_vote_candidate_projection,
                (
                    "self.wal_identity.is_exact()",
                    "self.matches_verified_height(verified)",
                    "PendingRuntimeEffectBinding::from_exact_recovered_next_wal_vote(",
                    "self.replay_evidence.project_recovered_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionV1 {",
                    "projection.is_exact(verified)",
                ),
            )
            require_tokens(
                runtime_path,
                "runtime-private recovered next-WAL-Vote candidate mint",
                runtime_source,
                (
                    "fn project_recovered_lifecycle_next_wal_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1::new()",
                    "RecoveredWalCandidateProjectionPermit::new()",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "WAL-bound recovered Broadcast-and-next-Sign projection",
                wal_recovery_source,
                (
                    "fn project_authenticated_signed_broadcast_and_sign(",
                    "next_sign.matches_verified_height(verified)",
                    "next_sign.matches_phase_vote_repair(self)",
                    "project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign)",
                    "combined.children_are_exact(verified)",
                ),
            )
            combined_cold_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                "impl RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {",
                "fn project_recovered_signed_broadcast(",
            )
            require_order(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "self.cold_adapter_authority_minted",
                    "self.children_are_exact(verified)",
                    "self.next_sign.project_cold_adapter_next_sign(",
                    "RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(",
                    "self.cold_adapter_authority_minted = true",
                    "candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)",
                    "self.next_sign.owns_spliced_candidate(candidates)",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "fn into_parts(",
                    "pub fn broadcast(",
                    "pub fn next_sign(",
                    "candidates.len() == 2",
                ),
            )
            next_vote_cold_projection = region(
                replay_authority_path,
                replay_authority_source,
                "sealed recovered next-WAL-Vote cold adapter projection",
                "pub(super) fn project_cold_adapter_next_sign(",
                "/// Return the exact installed effect digest",
            )
            require_order(
                replay_authority_path,
                "sealed recovered next-WAL-Vote cold adapter projection",
                next_vote_cold_projection,
                (
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "self.is_exact(verified)",
                    "self.seal.effect.clone()",
                ),
            )
            combined_cold_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {",
                "impl RecoveredLifecycleSignColdAdapterAuthorityV1",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "broadcast: AdapterEffect",
                    "next_sign: AdapterEffect",
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "ConsensusMessageV2Payload::Proposal(proposal)",
                    "ConsensusMessageV2Payload::Vote(vote)",
                    "GlobalPhase::Prepare => tag.view() == next_vote.round.view",
                    "GlobalPhase::Commit => tag.view() >= next_vote.round.view",
                    "relation_is_exact.then_some(Self",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "fn into_parts(",
                    "fn broadcast(",
                    "fn next_sign(",
                    "impl Clone for RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1",
                ),
            )
            combined_cold_adapter = region(
                adapter_path,
                adapter_source,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                "pub(in crate::sumeragi) fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
                "/// Seal every adapter-owned input required by the adjacent gate open.",
            )
            require_order(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                (
                    "verified.verify_consensus_message(message)",
                    "adapter.reducer.awaiting_signature()",
                    "next_reducer.step(event.clone())",
                    "replayed_broadcast != broadcast",
                    "replayed_next_sign != next_sign",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            reject_tokens(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                ("publish_status", ".append(", "broadcast_consensus", "enqueue("),
            )
            combined_ledger_classifier = region(
                ledger_operations_path,
                ledger_operations_source,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "/// Stage the exact all-row tombstone successor for finalized-height retirement.",
            )
            require_tokens(
                ledger_operations_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "index.unique_parent_index(broadcast_ordinal)",
                    "index.owner_record_count(next_sign_owner) != 1",
                    "index.has_incoming_edge(next_sign_ordinal)",
                    "let next_sign_ordinal = broadcast_ordinal.checked_add(1)?",
                    "signed_broadcast_continuation_is_exact(",
                    "recovered_broadcast_and_next_sign_keys_are_exact(",
                    "next_sign_owner.first_admission_ordinal() != next_sign_ordinal",
                    "parent_record_count == 2",
                    "parent_record_count == 3",
                    "DurableContinuationEdge::ValidateToSignPrepare",
                    "ledger_frame_identity",
                ),
            )
            combined_ledger_enumerator = region(
                ledger_operations_path,
                ledger_operations_source,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "fn project_recovered_lifecycle_signed_broadcast_and_sign_at(",
            )
            require_order(
                ledger_operations_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "self.records.iter()",
                    "project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
                    "&index",
                ),
            )
            require_token_count(
                ledger_operations_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                "self.frame_identity()",
                1,
            )
            reject_tokens(
                ledger_operations_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                ("high_water == next_sign_ordinal", "persist_exact_successor"),
            )
            combined_ledger_reauth = region(
                ledger_path,
                ledger_source,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                "pub(in crate::sumeragi) fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {",
                "/// Complete version-one durable lifecycle ledger.",
            )
            require_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                (
                    "project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)",
                    "== Some(self)",
                ),
            )
            reject_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                ("ledger.frame_identity()",),
            )
            combined_registry_prepare = region(
                registry_path,
                registry_source,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<",
                "impl<'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'_, 'adapter>",
            )
            require_order(
                registry_path,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "adapter.dispatch_key() != key",
                    "sign.matches_claimed_record(",
                    "adapter.project_broadcast_and_sign_authority(body)",
                    ".project_authenticated_signed_broadcast_and_sign(verified, projection_authority)",
                    "PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {",
                ),
            )
            reject_tokens(
                registry_path,
                "unpublished recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "ValidatedBodyReceipt",
                    "into_parts",
                    "entries.insert",
                    "entries.remove",
                    "persist_exact_successor",
                ),
            )
            combined_transition = region(
                body_pipeline_path,
                body_pipeline_source,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                "fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                "#[allow(clippy::too_many_arguments, clippy::too_many_lines)]\nfn stage_body_stage_transition_with_payload_relation(",
            )
            require_order(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "stage_recovered_lifecycle_sign_broadcast_transition(coordinator, lease, broadcast)",
                    "first.child_ordinal.checked_add(1)",
                    "staged.reduce_admit(AdmissionRequest::Candidate(next_sign))",
                    "next_sign_owner == broadcast_owner",
                    "staged.high_water != next_sign_ordinal",
                    "capacity_generation_before[&CapacityClass::Effect].saturating_add(1)",
                    "capacity_used_before[&CapacityClass::Consensus].saturating_add(1)",
                    "Ok(StagedRecoveredLifecycleSignBroadcastAndSignTransition {",
                ),
            )
            reject_tokens(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "persist_exact_successor",
                    "commit_after_publication",
                    "registry.entries",
                ),
            )
            combined_transition_publication = region(
                body_pipeline_path,
                body_pipeline_source,
                "durable recovered Broadcast-and-next-Sign publication",
                "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
                "fn map_sealed_successor_projection_error(",
            )
            require_order(
                body_pipeline_path,
                "durable recovered Broadcast-and-next-Sign publication",
                combined_transition_publication,
                (
                    "persist_exact_staged_successor(&self.staged)",
                    "successor.commit_after_publication()",
                    "*coordinator = staged",
                    "if publication_is_vote",
                    "ready_index.contains(&next_sign_ordinal)",
                    "adapter.commit_after_durable_vote_broadcast_and_sign()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "Proposal publication parks only its durable Broadcast debt",
                combined_transition_publication,
                (
                    "ready_index.remove(&broadcast_ordinal)",
                    "LifecycleState::Waiting(broadcast_wait)",
                    "adapter.commit_after_durable_broadcast_and_sign()",
                ),
            )
            combined_transition_tail = combined_transition_publication.split(
                "successor.commit_after_publication()", 1
            )[-1]
            reject_tokens(
                body_pipeline_path,
                "infallible recovered Proposal two-child publication tail",
                combined_transition_tail,
                ("return", "is_err", "Result"),
            )
            combined_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Proposal adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned recovered Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Proposal adapter two-child commit",
                combined_adapter_commit,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: true",
                    "persisted_prepare_wal",
                    "outbound_payload: Some(_)",
                    "adapter.pending_persistence_id = None",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                    "adapter.record_reducer_outcome(&persisted_event",
                ),
            )
            combined_vote_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Vote adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_vote_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned recovered Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Vote adapter two-child commit",
                combined_vote_adapter_commit,
                (
                    "self.is_vote_broadcast_and_sign()",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: false",
                    "outbound_payload: None",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            require_tokens(
                registry_validate_path,
                "follow-on recovered WAL Vote remains an executable Sign carrier",
                registry_validate_source,
                (
                    "ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)",
                    "PreparedRecoveredLifecycleSignCarrier::NextWalVote(sign)",
                ),
            )
            recovered_sign_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Sign-to-Broadcast settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
                "/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion(authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "durable recovered Sign-to-Broadcast settlement leaves output to its child",
                recovered_sign_settlement,
                (
                    "capture_recovered_lifecycle_signed_broadcast_refanout",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_sign_tail = recovered_sign_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Sign-to-Broadcast post-fsync tail",
                recovered_sign_tail,
                ("return", "Result", "is_err"),
            )
            recovered_vote_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
                "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                recovered_vote_two_child_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.is_vote_broadcast_and_sign_shape()",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                    "ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "Vote settlement leaves durable output to typed refanout",
                recovered_vote_two_child_settlement,
                (
                    "project_proposal_exact_output_authority",
                    "capture_recovered_lifecycle_proposal_exact_output",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_vote_two_child_tail = recovered_vote_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Vote two-child post-fsync tail",
                recovered_vote_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_proposal_prepare_wal_settlement = region(
                launch_path,
                launch_source,
                "restart-closed initial Proposal PrepareIntent settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(",
                "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
            )
            require_order(
                launch_path,
                "restart-closed initial Proposal PrepareIntent settlement",
                recovered_proposal_prepare_wal_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "preview.project_proposal_exact_output_authority()",
                    "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                    "output.prepare_wal_append_permit()",
                    "preview.append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "output.commit_after_publication()",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied",
                ),
            )
            require_tokens(
                launch_path,
                "initial Proposal capacity remains pre-WAL retryable",
                recovered_proposal_prepare_wal_settlement,
                (
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
                    "*recovered_lifecycle_sign_completion = Some(completion)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable",
                ),
            )
            reject_tokens(
                launch_path,
                "post-WAL initial Proposal never releases fail-stop output",
                recovered_proposal_prepare_wal_settlement.split(
                    "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)", 1
                )[-1],
                ("output.abort_before_publication()",),
            )
            recovered_proposal_prepare_wal_tail = recovered_proposal_prepare_wal_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible initial Proposal post-Ledger tail",
                recovered_proposal_prepare_wal_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_proposal_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
                "pub(in crate::sumeragi) fn drive_recovered_decision_apply_deferred(",
            )
            require_order(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.project_proposal_exact_output_authority()",
                    "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "output.commit_after_publication()",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied",
                ),
            )
            require_token_count(
                launch_path,
                "typed recovered Proposal pre-fsync output release",
                recovered_proposal_two_child_settlement,
                "output.abort_before_publication()",
                2,
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable",
                    "*recovered_lifecycle_sign_completion = Some(completion)",
                    "drop(output)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired",
                ),
            )
            recovered_proposal_two_child_tail = recovered_proposal_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Proposal two-child post-fsync tail",
                recovered_proposal_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_broadcast_refanout = region(
                scheduler_path,
                scheduler_source,
                "restart-safe recovered signed-Broadcast refanout",
                "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
                "fn persist_recovered_decision_fetch_response_after_runner(",
            )
            require_order(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "if exact_ready != self.coordinator.ready_index",
                    "work_class == LifecycleWorkClass::Broadcast",
                    "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
                    "attest_ready_recovered_lifecycle_signed_broadcast",
                    "for ready_ordinal in &exact_ready",
                    "attest_ready_recovered_lifecycle_sign(",
                    "self.coordinator.plan_turn(inputs)",
                    "project_claimed_recovered_lifecycle_signed_broadcast_output",
                    "capture_recovered_lifecycle_signed_broadcast_refanout(authority)",
                    "let wait_source = super::WaitSource::Recovery(wait_digest)",
                    "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
                    "output.commit_after_publication()",
                ),
            )
            require_tokens(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "rollback_unpublished_turn(&lease)",
                    "close_admission_for_restart()",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::CapacityUnavailable",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned",
                    "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(",
                ),
            )
            reject_tokens(
                scheduler_path,
                "volatile recovered signed-Broadcast refanout wait",
                recovered_broadcast_refanout,
                (
                    "persist_exact_successor",
                    "TurnOutcome::Terminal",
                    "exact_ready.len() == 2",
                    "exact_ready.len() != 2",
                ),
            )
            require_tokens(
                registry_validate_path,
                "retained recovered Broadcast-and-next-Vote pair seal",
                registry_validate_source,
                (
                    "fn recovered_lifecycle_signed_broadcast_declares_next_vote(",
                    "fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
                    "let (next, next_digest) = broadcast.paired_next_sign?",
                    "next_record.physical_slots.get(&next.slot) == Some(&next_digest)",
                    "self.recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal( coordinator, broadcast_ordinal, ) != Some(next_sign_ordinal)",
                    "DurableRecoveredLifecycleNextWalVoteSign(next_sign)",
                ),
            )
            require_tokens(
                worker_path,
                "durable recovered signed-Broadcast service capture",
                worker_source,
                (
                    "fn capture_recovered_lifecycle_signed_broadcast_refanout(",
                    "authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new())",
                    "PendingExactFanout::claimed(",
                    "pending.can_enqueue(fanout)",
                    "fn capture_recovered_lifecycle_cold_proposal_message(",
                    "output.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "self.proposal_work_retired",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "cold_durable_proposal_refanout_atomically_owns_control_and_chunks",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered signed-Broadcast ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_control_signed_broadcast(",
                    "fn authenticate_recovered_phase_signed_broadcast_repair(",
                    "project_recovered_signed_broadcast_child(self.context())",
                    "recover_durable_signed_broadcast(verified, child)",
                    "broadcast.exactly_matches_record(",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "cold recovered signed-Broadcast WAL and roster join",
                wal_recovery_source,
                (
                    "fn recover_durable_signed_broadcast(",
                    "verified.verify_consensus_message(message)",
                    "fn project_cold_adapter_authority(",
                    "RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(",
                ),
            )
            require_tokens(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                (
                    "fn advance_recovered_lifecycle_signed_broadcast(",
                    "verify_individual_signature(",
                    "let [reducer::Effect::Broadcast(message)] = core_effects.as_slice()",
                    "replayed != broadcast",
                    "next_reducer.pending_persistence_record().is_some()",
                    "next_reducer.awaiting_signature().is_some()",
                ),
            )
            require_literal_count(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                '"Proposal cold replay requires its body and Prepare WAL successor"',
                2,
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered signed-Broadcast storage census",
                lifecycle_open_source,
                (
                    "PhaseBroadcast(",
                    "PhaseBroadcastAndSign(",
                    "PhaseBroadcastAndNextSign(",
                    "ControlBroadcast(",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_sign_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_control_broadcast_and_durable_fetch_startup",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered phase Broadcast-and-Sign ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "combined.broadcast_exactly_matches(&broadcast)",
                    "combined.exactly_matches_fresh_records(",
                    "fn revalidates_recovered_phase_signed_broadcast_and_sign(",
                ),
            )
            require_tokens(
                registry_path,
                "cold recovered phase Broadcast-and-Sign registry join",
                registry_source,
                (
                    "#[inline(never)] pub(in crate::sumeragi) fn prepare_cold_adapter_startup(",
                    "Self::prepare_cold_sign_branch(",
                    "Self::prepare_cold_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_sign_branch(",
                    "#[inline(never)] fn prepare_cold_signed_broadcast_branch(",
                    "let pair_hint = matching.next()",
                    "if matching.next().is_some()",
                    "drop(matching)",
                    "Self::prepare_cold_signed_broadcast_and_next_vote_branch(",
                    "Self::prepare_cold_single_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_single_signed_broadcast_branch(",
                    "#[inline(never)] fn prepare_cold_signed_broadcast_and_next_vote_branch(",
                    "authenticate_recovered_lifecycle_next_vote_body(&mut preview)",
                    "project_authenticated_cold_signed_broadcast_and_sign(verified, seal)",
                    "authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "advance_recovered_lifecycle_signed_broadcast_and_sign(",
                    "#[inline(never)] pub(crate) fn install_recovered_wal_sign(",
                    "Self::install_recovered_sign_branch(",
                    "Self::install_recovered_broadcast_branch(",
                    "Self::install_recovered_broadcast_and_next_vote_branch(",
                    "#[inline(never)] fn install_recovered_sign_branch(",
                    "#[inline(never)] fn install_recovered_broadcast_branch(",
                    "#[inline(never)] fn install_recovered_broadcast_and_next_vote_branch(",
                    "fn install_recovered_broadcast_and_next_vote(",
                    "paired_next_sign: Some((next_sign_address, next_sign_digest))",
                    "fn phase_broadcast_and_next_vote_projection(",
                    "owns_recovered_phase_broadcast_and_next_sign(",
                ),
            )
            pair_install = _require_rust_item(
                registry_path,
                registry_source,
                "install_recovered_broadcast_and_next_vote",
                errors,
            )
            if pair_install is not None:
                require_tokens(
                    registry_path,
                    "cold recovered phase Broadcast-and-Sign registry join",
                    pair_install.source,
                    (
                        "paired_next_sign: Some((next_sign_address, next_sign_digest))",
                    ),
                )
            require_tokens(
                adapter_path,
                "cold recovered phase owner handoff",
                adapter_source,
                (
                    "#[inline(never)] fn authenticate_recovered_phase_vote_stage<'registry>(",
                    "Ok(Box::new(authenticated))",
                    "#[inline(never)] fn persist_recovered_phase_vote_stage<'registry>(",
                    "(*authenticated).persist_repair()",
                    "Ok(persisted)",
                    "#[inline(never)] fn prepare_recovered_phase_vote_cold_adapter_stage<'registry>(",
                    "local_proposal_attempt: Option<RecoveredLifecycleLocalProposalAttemptV1>",
                    "ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt( adapter, effects, local_proposal_attempt, )",
                    "prepare_cold_adapter_startup(&verified, adapter_startup, body_store)",
                    "ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup { adapter_startup, verified, persisted, }",
                    "#[inline(never)] fn install_recovered_phase_vote_sign_stage<'registry>(",
                    "(*prepared).install_recovered_sign()",
                    "#[inline(never)] fn open_recovered_phase_vote_seals_stage(",
                    "(*installed).open_production_owner_seals(",
                    "#[inline(never)] fn finish_recovered_phase_vote_owner_stage(",
                    "(*paired).into_owner(registry, payload_store, body_store)",
                ),
            )
            phase_branch = _require_rust_item(
                adapter_path,
                adapter_source,
                "open_recovered_phase_vote_branch",
                errors,
            )
            if phase_branch is not None:
                require_order(
                    adapter_path,
                    "cold recovered phase owner handoff",
                    phase_branch.source,
                    (
                        "Self::ensure_recovered_body_store_context(&body_store, &verified)",
                        "Self::open_recovered_non_apply_stores(",
                        "Self::authenticate_recovered_phase_vote_stage(",
                        "Self::persist_recovered_phase_vote_stage(authenticated)",
                        "Self::prepare_recovered_phase_vote_cold_adapter_stage( persisted, &body_store, local_proposal_attempt, )",
                        "Self::install_recovered_phase_vote_sign_stage(prepared)",
                        "Self::open_recovered_phase_vote_seals_stage(",
                        "Self::finish_recovered_phase_vote_owner_stage(",
                    ),
                )
            recovered_phase_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered phase-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup(",
                "/// Assemble the exact standalone control Sign with every durable Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered phase-Broadcast storage assembly",
                recovered_phase_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            recovered_control_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered control-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_control_broadcast_and_durable_fetch_startup(",
                "/// Assemble the standalone Decision Fetch with every durable body-backed Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered control-Broadcast storage assembly",
                recovered_control_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            require_tokens(
                worker_path,
                "dedicated recovered Sign queue ownership",
                worker_source,
                (
                    "recovered_lifecycle_signs:",
                    "BTreeMap<RecoveredLifecycleSignDispatchKeyV1, V2IoTrackedRecoveredLifecycleSignV1>",
                    "fn transfer_recovered_lifecycle_sign_completion_at(",
                    "io.prepare_recovered_lifecycle_sign_completion(guarded, ownership_position)",
                    "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
                    "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
                    "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
                ),
            )
            recovered_sign_capacity = region(
                worker_path,
                worker_source,
                "recovered Sign capacity capture release",
                "fn capture_recovered_lifecycle_sign_capacity<'a>(",
                "fn recovered_completion_worker_capacity(",
            )
            require_token_count(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                "operation.complete()",
                4,
            )
            reject_tokens(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                ("drop(operation)",),
            )
            rollback_unpublished = region(
                owner_path,
                owner_source,
                "unpublished recovered Sign claim rollback",
                "fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {",
                "/// Rebuild records after seeding the ordinal high-water mark.",
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                (
                    "lease.output_reservation.is_some()",
                    "assert!( inserted,",
                    "self.active_lease = None",
                ),
            )
            reject_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                ("debug_assert!",),
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign rollback regression",
                owner_source,
                (
                    "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
                ),
            )
            launched_owner_fields = region(
                launch_path,
                launch_source,
                "launched recovered Sign Drop order",
                "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
                "/// Result of draining one dedicated recovered Apply worker completion.",
            )
            require_order(
                launch_path,
                "launched recovered Sign Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            recovered_fetch_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Decision Fetch dispatch",
                "fn dispatch_completion_with_runner_debt(",
                "/// Reserve, claim, and dispatch the sole Ready lifecycle-owned recovered Sign.",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "attest_ready_recovered_decision_fetch",
                    "authenticate_recovered_decision_fetch_request(",
                    "take_request_authority()",
                    "capture_recovered_completion_capacity_census(probes)",
                    "self.coordinator.plan_turn(inputs)",
                    "census.select_fetch(ordinal)",
                    "prepare_recovered_decision_fetch_request_registration(owner)",
                    "prepare_recovered_decision_fetch_dispatch",
                    "registration.commit(prepared, wait_source)",
                    "output.commit()",
                ),
            )
            require_tokens(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor",
                    "RecoveredCompletionCapacityProbeV1::Fetch",
                    "authenticated_capacity(ordinal, &factory)",
                    "prepared.dispatch_key() != registration.dispatch_key()",
                    "installed != dispatch_key",
                ),
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Decision Fetch request dispatch",
                recovered_fetch_dispatch,
                (
                    "EffectWorkId",
                    "RuntimeEffectOwnership",
                    "PendingRuntimeEffectBinding",
                    "into_parts",
                    "settle",
                ),
            )
            recovered_fetch_phase_a = region(
                scheduler_path,
                scheduler_source,
                "recovered Decision Fetch response persistence Phase A",
                "fn persist_recovered_decision_fetch_response_after_runner(",
                "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
            )
            require_order(
                scheduler_path,
                "recovered Decision Fetch response persistence Phase A",
                recovered_fetch_phase_a,
                (
                    "capture_lifecycle_capacity_rank(selector)",
                    "reservation.preflight_recovered_decision_fetch_target_absent()",
                    "executor.prepare_recovered_decision_fetch_body_persistence(prepared)",
                    "reservation.preflight_recovered_decision_fetch_body_persistence(&task)",
                    "executor.prepare_recovered_decision_fetch_response_claim(&task)",
                    "let mut next = self.coordinator.stage_durable_transaction()",
                    "next.plan_turn(inputs)",
                    "matches_claimed_dispatched_recovered_decision_fetch(",
                    "self.coordinator = next",
                    "claim.commit_with_queue(reservation, task)",
                    "assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))",
                ),
            )
            recovered_fetch_ingress = region(
                turn_driver_path,
                turn_driver_source,
                "unified recovered Decision Fetch ingress driver",
                "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
                "fn drive_recovered_ingress_selector<'cursor>(",
            )
            require_order(
                turn_driver_path,
                "unified recovered Decision Fetch ingress driver",
                recovered_fetch_ingress,
                (
                    "if !self.runner_turn_matches(",
                    "LifecycleRunnerRankTarget::Ingress",
                    "return ProductionLifecycleIngressTurnV1::PassThrough(runner)",
                    "self.drive_recovered_ingress_selector(selector, runner)",
                ),
            )
            recovered_fetch_ingress_handoff = region(
                turn_driver_path,
                turn_driver_source,
                "validated recovered Decision Fetch Phase-A handoff",
                "fn drive_recovered_ingress_selector<'cursor>(",
                "fn settle_parked_recovered_sign_completion(",
            )
            require_order(
                turn_driver_path,
                "validated recovered Decision Fetch Phase-A handoff",
                recovered_fetch_ingress_handoff,
                (
                    "persist_recovered_decision_fetch_response_after_runner(",
                    "drop(runner)",
                    "ProductionLifecycleIngressTurnV1::Selected(selected)",
                ),
            )
            require_tokens(
                launch_path,
                "recovered Decision Fetch source-order regression",
                launch_source,
                (
                    "fn recovered_decision_fetch_phase_a_is_reachable_only_after_runner_validation()",
                ),
            )
            recovered_fetch_ready = region(
                registry_validate_path,
                registry_validate_source,
                "closed Ready and claimed recovered Decision Fetch carrier",
                "pub(super) fn attest_ready_recovered_decision_fetch(",
                "/// Project a comparison-only seal for this exact registry instance.",
            )
            require_tokens(
                registry_validate_path,
                "closed Ready and claimed recovered Decision Fetch carrier",
                recovered_fetch_ready,
                (
                    "fetch.dispatch_key.is_some()",
                    "fetch.matches_current_ready_record(address, digest, coordinator)",
                    "RecoveredDecisionFetchDispatchIdentityV1::new(",
                    "project_recovered_decision_fetch_request(identity)",
                    "fn matches_claimed_dispatched_recovered_decision_fetch(",
                    "fetch.dispatch_key == Some(key)",
                    "fetch.matches_claimed_record(address, digest, coordinator, lease)",
                    "fn prepare_recovered_decision_fetch_dispatch(",
                ),
            )
            recovered_fetch_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "payload-free recovered Decision Fetch projection",
                "pub(super) fn project_recovered_decision_fetch_request(",
                "/// Prove the authenticated recovery cut retains this exact Fetch.",
            )
            require_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                (
                    "AdapterEffect::FetchBody {",
                    "manifest: None",
                    "certificate: Some(certificate)",
                    "RecoveredDecisionFetchRequestAuthorityV1::from_registry_projection(",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            recovered_fetch_registration = region(
                effects_path,
                effects_source,
                "dedicated recovered Decision Fetch request owner census",
                "pub(in crate::sumeragi) fn recovered_decision_fetch_registration_available(",
                "/// Take ownership of an exact-body store opened during sealed preflight.",
            )
            require_tokens(
                effects_path,
                "dedicated recovered Decision Fetch request owner census",
                recovered_fetch_registration,
                (
                    "self.validated_certified_request_presence().is_err()",
                    "self.outstanding_requests.len().checked_add(self.recovered_decision_fetches.len())",
                    "owner.conflicts_with_ordinary_tracker(&self.outstanding_requests)",
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_registration(",
                    "PreparedRecoveredDecisionFetchRequestRegistrationV1 { executor: self, owner: Some(owner), }",
                ),
            )
            require_tokens(
                effects_path,
                "complete recovered Decision Fetch request census and terminal fence",
                effects_source,
                (
                    "recovered_decision_fetches: BTreeMap<",
                    "recovered_decision_fetch_by_request: BTreeMap<",
                    "fn recovered_decision_fetch_request_index_is_exact_and_empty(&self) -> bool",
                    "self.recovered_decision_fetch_request_index_is_exact_and_empty()",
                    "fn validated_certified_request_presence(",
                    "Ok(!pending_hashes.is_empty() || !recovered_hashes.is_empty())",
                ),
            )
            ordinary_fetch_admission = region(
                effects_path,
                effects_source,
                "ordinary and recovered Decision Fetch coordinate fence",
                "fn begin_fetch<S: V2EffectServices>(",
                "fn retained_body_manifest_hash(",
            )
            require_tokens(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                (
                    "self.recovered_decision_fetches.values()",
                    "owner.matches_body_coordinates(round, subject)",
                ),
            )
            require_literal_count(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                '"body-fetch coordinates already have a recovered Decision Fetch owner"',
                1,
            )
            require_tokens(
                effects_path,
                "symmetric recovered Decision Fetch owner census",
                effects_source,
                (
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
                    "executor.validated_certified_request_presence()",
                ),
            )
            recovered_fetch_selector = region(
                selector_path,
                selector_source,
                "typed recovered Decision Fetch selector consumption",
                "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(",
                "/// Consume one exact selected family into a bounded body-store command.",
            )
            require_order(
                selector_path,
                "typed recovered Decision Fetch selector consumption",
                recovered_fetch_selector,
                (
                    "self.revalidate_recovered_decision_fetch_response_candidate(",
                    "PreparedCertifiedResponseCandidate::Recovered(candidate)",
                    "let authenticated = candidate.into_authenticated_response()",
                    "RecoveredDecisionFetchBodyPersistenceTaskV1 {",
                ),
            )
            require_tokens(
                selector_path,
                "typed recovered Decision Fetch selector target",
                selector_source,
                (
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    "LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence",
                    "fn matches_recovered_decision_fetch_key(",
                ),
            )
            require_tokens(
                worker_path,
                "typed recovered Decision Fetch selector target consumer",
                worker_source,
                (
                    "target.matches_recovered_decision_fetch_key(task.dispatch_key())",
                ),
            )
            recovered_fetch_next_selector = region(
                selector_path,
                selector_source,
                "queue-owned recovered Decision Fetch selector",
                "pub(crate) fn prepare_next_recovered_decision_fetch_ingress_selector(",
                "/// Classify every exact pre-cut fair-ingress occurrence without mutation.",
            )
            require_order(
                selector_path,
                "queue-owned recovered Decision Fetch selector",
                recovered_fetch_next_selector,
                (
                    "self.lifecycle_terminal_subject()",
                    "capture_next_lifecycle_queue_cut(",
                    "v2_ingress_head_can_drain(occurrence.inbound(), self, terminal_subject)",
                    "self.capture_lifecycle_ingress_selector(cut)",
                    "prepared.queue_witness.selected_disposition()",
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    ".selected_claimed_response_family()",
                ),
            )
            reject_tokens(
                selector_path,
                "queue-owned recovered Decision Fetch selector",
                recovered_fetch_next_selector,
                (
                    "target_physical_ordinal:",
                    "prepare_lifecycle_ingress_selector(",
                    "try_recv",
                    "commit_exact_dequeue",
                ),
            )
            recovered_fetch_queue_cut = region(
                ingress_position_path,
                ingress_position_source,
                "queue-owned recovered Decision Fetch fair cut",
                "pub(super) fn capture_next_lifecycle_queue_cut(",
                "fn capture_lifecycle_queue_cut_for(",
            )
            require_tokens(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair cut",
                recovered_fetch_queue_cut,
                (
                    "LifecycleQueueCutTarget::NextAdmissible",
                    "predicate: impl FnMut(&FairIngressSelectorOccurrence) -> bool",
                    "Result<Option<FairIngressQueueCut<'_>>, FairIngressQueueCutError>",
                ),
            )
            recovered_fetch_fair_selection = region(
                ingress_position_path,
                ingress_position_source,
                "queue-owned recovered Decision Fetch fair selection",
                "fn select_next_admissible_ordinal(",
                "fn mint_pending_identities(",
            )
            require_order(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair selection",
                recovered_fetch_fair_selection,
                (
                    "geometry.ready_prefix.iter()",
                    "selector.queue_gate() != occurrence.value.queue_gate",
                    "select_fair_v2_ingress_candidate(",
                    "occurrence.physical_admission_ordinal()",
                    "occurrence.queue_gate()",
                    "occurrence.is_obsolete()",
                    "predicate(occurrence)",
                ),
            )
            reject_tokens(
                ingress_position_path,
                "queue-owned recovered Decision Fetch fair selection",
                recovered_fetch_fair_selection,
                ("pop_", "remove(", "rotate_", "dequeue_selected_locked"),
            )
            shared_fair_selection = region(
                sumeragi_path,
                sumeragi_source,
                "shared strict-then-dependency fair selection",
                "fn select_fair_v2_ingress_candidate<T>(",
                "/// Fixed-capacity, roster-aware v2 ingress with per-hop admission and service fairness.",
            )
            require_order(
                sumeragi_path,
                "shared strict-then-dependency fair selection",
                shared_fair_selection,
                (
                    "for dependency_pass in [false, true]",
                    "for (source_index, source_candidates) in candidates.iter().enumerate()",
                    "for candidate in source_candidates",
                    "gate == FairV2IngressQueueGateVerdict::Blocked",
                    "dependency != dependency_pass",
                    "obsolete || predicate(candidate)",
                    "return Some((source_index, ordinal, disposition))",
                ),
            )
            ordinary_fair_dequeue = region(
                sumeragi_path,
                sumeragi_source,
                "ordinary shared fair selection call",
                "fn try_recv_if_at_checked_classified(",
                "/// Commit one already selected occurrence",
            )
            require_tokens(
                sumeragi_path,
                "ordinary shared fair selection call",
                ordinary_fair_dequeue,
                ("select_fair_v2_ingress_candidate(",),
            )
            require_tokens(
                effects_path,
                "shared pure ingress drain predicate",
                effects_source,
                (
                    "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
                    "certified_body_request_is_superseded_after_decision(",
                    "executor.can_admit_network_message_with_ingress_ownership(",
                ),
            )
            require_tokens(
                runner_path,
                "ordinary runner shared ingress drain predicate",
                runner_source,
                ("v2_ingress_head_can_drain(inbound, executor, terminal_subject)",),
            )
            require_tokens(
                effects_path,
                "queue-owned recovered Decision Fetch selector behavior",
                effects_source,
                (
                    "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
                    ".prepare_next_recovered_decision_fetch_ingress_selector(&ingress)",
                ),
            )
            for literal in (
                '"a later recovered response cannot leapfrog the ordinary fair winner"',
                '"the queue-owned selector chooses the next fair exact family occurrence"',
                '"queue-owned selector discovery cannot dequeue or renumber ingress"',
            ):
                require_literal_count(
                    effects_path,
                    "queue-owned recovered Decision Fetch selector behavior",
                    effects_source,
                    literal,
                    1,
                )
            recovered_fetch_claim = region(
                effects_path,
                effects_source,
                "recovered Decision Fetch response claim publication",
                "pub(in crate::sumeragi) fn commit_with_queue(",
                "impl RecoveredDecisionFetchResponseCandidateV1",
            )
            require_order(
                effects_path,
                "recovered Decision Fetch response claim publication",
                recovered_fetch_claim,
                (
                    "owner.matches_response_claim_preflight(response_hash, preflight)",
                    "owner.commit_exact_response_claim(response_hash)",
                    "queue.commit_recovered_decision_fetch_body_persistence(task)",
                ),
            )
            recovered_fetch_mixed_head = region(
                worker_path,
                worker_source,
                "recovered Decision Fetch mixed completion head fence",
                "fn take_io_completion(&mut self, runtime_capacity_available: bool)",
                "fn take_recovered_lifecycle_sign_completion(",
            )
            require_order(
                worker_path,
                "recovered Decision Fetch mixed completion head fence",
                recovered_fetch_mixed_head,
                (
                    "let ownership_position =",
                    "io.completion_ownership_at(ownership_position)",
                    "owned.recovered_decision_fetch.is_some()",
                    "return IoCompletionTake::retained_runtime()",
                    "io.try_recv_completion_unacknowledged()",
                ),
            )
            recovered_fetch_classifier = region(
                worker_path,
                worker_source,
                "unified recovered Decision Fetch completion classifier",
                "pub(in crate::sumeragi) fn take_next_lifecycle_completion(",
                "/// Drain only the oldest recovered-Sign guard;",
            )
            require_order(
                worker_path,
                "unified recovered Decision Fetch completion classifier",
                recovered_fetch_classifier,
                (
                    "V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded)",
                    "prepare_recovered_decision_fetch_body_completion(guarded, 0)",
                    "LifecycleCompletionTakeV1::DecisionFetch(",
                ),
            )
            require_tokens(
                worker_path,
                "unified recovered Decision Fetch worker ownership",
                worker_source,
                (
                    "PersistRecoveredDecisionFetchBody(RecoveredDecisionFetchBodyPersistenceTaskV1)",
                    "recovered_decision_fetch_bodies: BTreeMap<RecoveredDecisionFetchDispatchKeyV1, V2IoTrackedRecoveredDecisionFetchBodyV1>",
                    "V2IoCompletion::RecoveredDecisionFetchBodyPersisted",
                    "V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained",
                    "fn take_next_lifecycle_completion(",
                    "fn recovered_decision_fetch_queue_transitions_and_parks_until_dedicated_extraction()",
                ),
            )
            parked_fetch_completion = region(
                worker_path,
                worker_source,
                "opaque parked recovered Decision Fetch completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchBodyCompletionV1 {",
                "impl PreparedRecoveredLifecycleSignCompletionV1",
            )
            reject_tokens(
                worker_path,
                "opaque parked recovered Decision Fetch completion",
                parked_fetch_completion,
                (
                    "fn into_parts(",
                    "fn durable_receipt(",
                    "fn response(",
                    "fn acknowledge(",
                    "fn settle(",
                ),
            )
            recovered_fetch_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
                "/// Reserve, claim, and queue one recovered Sign",
            )
            require_order(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "prepare_lifecycle_ingress_selector(",
                    "prepare_recovered_decision_fetch_owner_retirement(",
                    "into_locked_recovered_decision_fetch_dequeue(",
                    "prepare_recovered_decision_fetch_store_adapter_authority(",
                    "prepare_recovered_decision_fetch_store_adapter(",
                    "prepare_recovered_decision_fetch_store_successor(",
                    "prepare_recovered_decision_fetch_store_transition(",
                    "begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "commit_recovered_decision_fetch_owner_retirement(retirement)",
                    "locked_dequeue.commit()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "*recovered_decision_fetch_body_completion = Some(completion)",
                    "owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure)",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "dedicated recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch worker acknowledgement tail",
                region(
                    worker_path,
                    worker_source,
                    "recovered Decision Fetch worker acknowledgement tail",
                    "fn acknowledge_recovered_decision_fetch_body(",
                    "fn prepare_certified_fetch_body_persistence_ack(",
                ),
                (
                    "fn acknowledge_recovered_decision_fetch_body(",
                    ".recovered_decision_fetch_bodies",
                    ".remove(&key)",
                ),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch guarded acknowledgement tail",
                worker_source,
                (
                    "fn acknowledge_after_publication(mut self)",
                    "self.drop_guard.disarm()",
                ),
            )
            require_tokens(
                ledger_path,
                "recovered Decision Store cold restart and marker-prefix closure",
                ledger_source,
                (
                    "fn authenticate_recovered_decision_fetch_store(",
                    "fn open_recovered_decision_store_startup(",
                    "fn stage_recovered_decision_apply_projection(",
                    "successor_records_after_live_store(",
                    "fn recovered_decision_store_crash_prefix_restarts_once_then_stutters()",
                    "fn recovered_decision_store_restart_rejects_an_exact_child_key_collision()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "recovered Decision Fetch payload-free parent transition",
                body_pipeline_source,
                (
                    "fn stage_recovered_decision_fetch_store_transition(",
                    "DurablePayloadReference::None",
                    "DurableContinuationEdge::FetchToStore",
                    "BodyStagePayloadRelationV1::RecoveredDecisionFetch",
                    "fn persist_exact_successor(",
                    "fn commit_after_publication(self)",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Decision Store cold adapter reconstruction",
                adapter_source,
                (
                    "fn advance_recovered_decision_fetch_store(",
                    "project_store_adapter_authority(body)",
                    "project_decision_fetch_store(verified, projection_body, preview.store_effect())",
                    "preview.commit_after_durable_settlement()",
                ),
            )
            require_tokens(
                body_store_path,
                "recovered Decision Store body-frame reconstruction",
                body_store_source,
                (
                    "struct RecoveredDecisionFetchStoreBodyAuthorityV1",
                    "fn recovered_decision_fetch_store_body(",
                    "Ok(RecoveredDecisionFetchStoreBodyAuthorityV1 { manifest: manifest.clone(), durable: durable.clone(), })",
                ),
            )
            require_tokens(
                lifecycle_open_path,
                "typed recovered Decision Store storage census",
                lifecycle_open_source,
                (
                    "RecoveredWalStartupProjectionV1::DecisionStore",
                    "assemble_storage_only_with_recovered_decision_store_and_durable_fetch_startup",
                    "recovered_decision_store_chain_records(",
                ),
            )
            require_tokens(
                registry_validate_path,
                "dedicated recovered Decision Store registry install",
                registry_validate_source,
                (
                    "RecoveredWalRegistrySlotV1::DecisionStore",
                    "fn install_recovered_wal_decision_store<'registry>(",
                    "ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore",
                ),
            )
            require_order(
                launch_path,
                "launched recovered Decision Fetch Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "recovered_decision_fetch_body_completion: Option<PreparedRecoveredDecisionFetchBodyCompletionV1>",
                    "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            request_scoped_response = region(
                transport_path,
                transport_source,
                "request-scoped certified response authentication",
                "pub(in crate::sumeragi) fn authenticate_response(",
                "/// Certified-body response admitted for one outstanding exact request.",
            )
            require_tokens(
                transport_path,
                "request-scoped certified response authentication",
                request_scoped_response,
                (
                    "authenticate_certified_body_response_for_request(",
                    "response.validate_against(",
                    "verify_signature(",
                    "decode_framed_signed_block(&response.body)",
                    "AuthenticatedCertifiedBodyResponse { response }",
                ),
            )
            require_tokens(
                kura_path,
                "process-local Kura identity seal",
                kura_source,
                (
                    "instance_identity: Arc<KuraInstanceIdentityMarker>",
                    "struct KuraInstanceIdentity(Arc<KuraInstanceIdentityMarker>)",
                    "Arc::ptr_eq(&self.0, &kura.instance_identity)",
                    "Arc::ptr_eq(&self.0, &other.0)",
                    "fn instance_identity(&self) -> KuraInstanceIdentity",
                    "fn instance_identity_names_only_the_exact_live_kura()",
                    "store_root_directory: BoundProgressDirectory",
                    "Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?",
                ),
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
                    "executor.ready_to_finish()",
                    "exactly_covers_finalization_work",
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
                    "recovered_decision_fetch_body_completion.is_some()",
                    "recovered_lifecycle_sign_completion.is_some()",
                    "completion_observer_activation.is_some()",
                    "ProductionLifecycleFinalizationErrorV1::NotReady",
                    "finalized_adapter: finalized",
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
            finalization_publication = region(
                ledger_path,
                ledger_source,
                "opaque all-row finalization publication",
                "fn persist_exact_finalization_successor(",
                "#[cfg(test)]",
            )
            require_order(
                ledger_path,
                "opaque all-row finalization publication",
                finalization_publication,
                (
                    "self,",
                    "StagedFinalizationRetirementV1 { current, retired }",
                    "LifecycleLedgerV1::from_coordinator(&self)? != current",
                    "store.persist_exact_successor(&current, &retired)?",
                    "store.load()? != retired",
                    "coordinator: self",
                ),
            )
            require_tokens(
                ledger_path,
                "opaque all-row finalization ownership",
                ledger_source,
                (
                    "struct StagedFinalizationRetirementV1 { current: LifecycleLedgerV1, retired: LifecycleLedgerV1, }",
                    "struct PublishedFinalizationRetirementV1 { coordinator: LifecycleCoordinator, current: LifecycleLedgerV1, retired: LifecycleLedgerV1, retained_floor: PublishedFinalizedLifecycleRetainedFloorV1, }",
                    "fn consume_owners( self, mut registry: LifecycleWorkRegistryHolder, )",
                    "registry.registry_mut().exactly_covers_finalization_work(&self.coordinator)",
                    "let retained_floor = self.retained_floor",
                    "drop(self.coordinator)",
                    "retained_floor",
                ),
            )
            reject_tokens(
                ledger_path,
                "opaque all-row finalization ownership",
                ledger_source,
                (
                    "impl Clone for StagedFinalizationRetirementV1",
                    "impl Copy for StagedFinalizationRetirementV1",
                    "impl Clone for PublishedFinalizationRetirementV1",
                    "impl Copy for PublishedFinalizationRetirementV1",
                    "pub coordinator: LifecycleCoordinator",
                    "pub current: LifecycleLedgerV1",
                    "pub retired: LifecycleLedgerV1",
                ),
            )
            require_tokens(
                lifecycle_startup_test_path,
                "production lifecycle all-row finalization behavior",
                lifecycle_startup_test_source,
                (
                    "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
                    ".retire_lifecycle_stores_for_test(finality_receipt)",
                    "cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)",
                    "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network()",
                    "let placeholder_cadence = exact_state.sumeragi_block_cadence()",
                    "placeholder_cadence.checked_add(Duration::from_millis(1))",
                    "assert_eq!(cadence_inputs.block_cadence, authenticated_cadence)",
                    "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
                    ".claim_producer_turn_for_local_proposal(&mut serve_runner)",
                    ".settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
                    "super::super::v2_runner::lifecycle_run_inner::finalize_lifecycle_height(",
                    "assert_eq!(receipt.context_id(), recovered_context.id())",
                    "assert_eq!(artifact.subject, subject)",
                    "Ok::<_, super::super::v2_runner::V2RunnerError>((successor, ()))",
                    "outcome.cleanup().warnings().is_empty()",
                    "outcome.wal_retirement_warning().is_none()",
                ),
            )
            finalization_registry = region(
                registry_validate_path,
                registry_validate_source,
                "finalization-only recovered registry census",
                "fn exactly_covers_finalization_work(",
                "fn exactly_covers_ready_work_with_extra(",
            )
            require_tokens(
                registry_validate_path,
                "finalization-only recovered registry census",
                finalization_registry,
                (
                    "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
                    "self.exact_recovered_wal_registry_slot()",
                    "self.exactly_covers_ready_work_with_extra(coordinator, extra, None, true)",
                ),
            )
            finalization_pair_link = region(
                registry_validate_path,
                registry_validate_source,
                "finalization recovered Broadcast pair link",
                "fn exact_optional_recovered_wal_authority(",
                "/// Install one work value without overwriting an incumbent address.",
            )
            require_tokens(
                registry_validate_path,
                "finalization recovered Broadcast pair link",
                finalization_pair_link,
                (
                    "broadcast.is_unpaired()",
                    "carrier.pairs_exact_next_sign(next_sign, next_sign_digest)",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "volatile refanned Broadcast finalization state",
                wal_recovery_source,
                (
                    "fn matches_current_finalization_record(",
                    "WaitSource::Recovery(digest)",
                    "coordinator.observed_generation.get(&expected_source) == Some(&wait.observed_generation())",
                    "!coordinator.ready_index.contains(&address.ordinal)",
                ),
            )
            require_tokens(
                scheduler_path,
                "volatile refanned Broadcast finalization behavior",
                scheduler_source,
                (
                    "fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign()",
                    "finalization_registry_census_is_exact_for_test()",
                ),
            )
            for literal in (
                '"finalization accepts the exact volatile refanout wait beside its Ready next Sign"',
                '"finalization must reject the corrupted exact next-Sign link"',
            ):
                require_literal_count(
                    scheduler_path,
                    "volatile refanned Broadcast finalization behavior",
                    scheduler_source,
                    literal,
                    1,
                )
        snapshot_authority = region(
            recovery_path,
            recovery_source,
            "SnapshotSuccessorActivationAuthority::new",
            "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
            "\n    /// Imported snapshot height which anchors the first executable context.",
        )
        require_tokens(
            recovery_path,
            "SnapshotSuccessorActivationAuthority::new",
            snapshot_authority,
            (
                "record.context.snapshot_bootstrap.as_ref()",
                "expect(\"verified snapshot activation authority retains its anchor\")",
                "record_hash: HashOf::new(record), snapshot_height: anchor.snapshot_height, snapshot_block_hash: anchor.snapshot_block_hash, successor_context_id: record.context.id(),",
            ),
        )
        recovery = region(
            recovery_path,
            recovery_source,
            "recover_active_height_with_plan",
            "pub(crate) fn recover_active_height_with_plan(",
            "\nfn verify_state_kura_prefix(",
        )
        require_tokens(
            recovery_path,
            "recover_active_height_with_plan snapshot authority",
            recovery,
            (
                "authenticate_v2_snapshot_replay_boundary(kura, state, &replay_plan)?;",
                "if record.context() != &bootstrap.context || record.proofs_of_possession() != bootstrap.validator_set_pops",
                "let verified_context = VerifiedHeightContext::snapshot_bootstrap(bootstrap)?;",
                "RecoveredSuccessorActivationAuthority::SnapshotBootstrap( SnapshotSuccessorActivationAuthority::new(bootstrap), )",
            ),
        )
        require_order(
            recovery_path,
            "recover_active_height_with_plan snapshot authority",
            recovery,
            (
                "authenticate_v2_snapshot_replay_boundary(",
                "is_entirely_audited_snapshot_import()",
                "authenticated_snapshot_v2_bootstrap()",
                "record.context() != &bootstrap.context",
                "VerifiedHeightContext::snapshot_bootstrap(bootstrap)",
                "SnapshotSuccessorActivationAuthority::new(bootstrap)",
            ),
        )
        require_tokens(
            recovery_path,
            "recover_active_height_with_plan complete-tip authority",
            recovery,
            (
                "kura.v2_finality_artifact_with_receipt(durable_height)?",
                "let predecessor_record = context_store.load(durable_height)?",
                "let verified_predecessor = verify_persisted_height( kura, state, &context_store, predecessor_record, durable_height, )?;",
                "let predecessor_signature_policy = if durable_height == 1 { BlockSignaturePolicy::GenesisAuthority(genesis_public_key.clone()) } else { BlockSignaturePolicy::RotatingLeader };",
                "build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;",
                "let (verified_context, activation) = successor.into_parts();",
                "RecoveredCompleteTipActivationAuthority::authenticate( parent_artifact, parent_receipt, verified_predecessor, predecessor_signature_policy, &verified_context, activation, kura, )?;",
                "RecoveredSuccessorActivationAuthority::CompleteTip( complete_tip_activation, )",
            ),
        )
        require_order(
            recovery_path,
            "recover_active_height_with_plan complete-tip authority",
            recovery,
            (
                "verify_persisted_height(",
                "build_verified_successor(",
                "successor.into_parts()",
                "RecoveredCompleteTipActivationAuthority::authenticate(",
                "RecoveredSuccessorActivationAuthority::CompleteTip(",
            ),
        )
        verified_successor = region(
            recovery_path,
            recovery_source,
            "build_verified_successor",
            "pub(crate) fn build_verified_successor(",
            "\nfn verify_persisted_height(",
        )
        require_tokens(
            recovery_path,
            "build_verified_successor",
            verified_successor,
            (
                "DurableV2PredecessorIdentity::authenticate(parent_artifact, parent_receipt)?;",
                "if state_height != parent_height || state_block_hash != Some(predecessor.block_hash)",
                "if parent_record.context() != &parent_artifact.height_context",
                "VerifiedHeightContext::successor( expected, proofs, parent_artifact, parent_receipt, parent_record.proofs_of_possession(), )?;",
                "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified.context().id(), }",
                "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified_context.context().id(), }",
            ),
        )
        require_order(
            recovery_path,
            "build_verified_successor",
            verified_successor,
            (
                "DurableV2PredecessorIdentity::authenticate(",
                "state_height != parent_height",
                "parent_record.context() != &parent_artifact.height_context",
                "VerifiedHeightContext::successor(",
                "DurableSuccessorActivationAuthority",
            ),
        )
    return errors
