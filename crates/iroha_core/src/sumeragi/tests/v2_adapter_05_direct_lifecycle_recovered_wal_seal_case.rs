#[test]
fn recovered_wal_vote_sign_seal_is_move_only_exact_and_unwired() {
    let source = include_str!("../v2.rs");
    let (production, _) = source
        .split_once("\n#[cfg(test)]\nmod tests {")
        .expect("locate unconditional production/test boundary");
    let token_start = production
        .find("// RECOVERED_WAL_VOTE_SIGN_SEAL_BEGIN")
        .expect("locate recovered WAL vote-sign token");
    let token_end = production[token_start..]
        .find("// RECOVERED_WAL_VOTE_SIGN_SEAL_END")
        .map(|offset| token_start + offset)
        .expect("locate end of recovered WAL vote-sign token");
    let token = &production[token_start..token_end];
    for required in [
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub(crate) struct RecoveredWalFrameIdentity",
        "pub(crate) struct RecoveredWalFrameIdentity",
        "frame_sequence: u64",
        "persistence_id: u64",
        "frame_hash: [u8; 32]",
        "fn from_recovered_record(record: &RecoveredRecord, persistence_id: u64)",
        "pub(crate) const fn is_exact(self) -> bool",
        "pub(crate) const fn persisted_locator(self) -> PersistedWalFrameLocatorV1",
        "pub(crate) struct PersistedWalFrameLocatorV1",
        "Decoding this value establishes only a structural locator",
        "pub(crate) struct RecoveredWalVoteSign",
        "wal_identity: RecoveredWalFrameIdentity",
        "replay_evidence: RecoveredWalVoteReplayEvidenceV1",
        "pub(crate) fn replay_evidence_is_exact(&self) -> bool",
        "tag: reducer::EventTag",
        "vote: wire::Vote",
        "prepare_certificate: Option<wire::QuorumCertificate>",
        "pub(crate) struct RecoveredAdapterStartup",
        "pub(crate) struct AuthenticatedRecoveredAdapterStartup",
        "pub(crate) struct ProductionLifecycleAdapterStartupV1",
        "struct AuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "struct StorageAuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "ledger: OpenedRecoveredWalValidateLedger",
        "struct PersistedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "struct ColdPreparedStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "struct InstalledStorageAuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "struct ProductionRecoveredLifecycleOwnerStartupV1",
        "ProductionRecoveredLifecycleOwnerAssemblyPermitV1",
        "struct DurableAuthenticatedRecoveredWalLifecycleStartup<'registry>",
        "struct InstalledRecoveredWalLifecycleStartup<'registry>",
        "struct RecoveredWalLifecycleLedgerPersistError<'registry>",
        "struct RecoveredWalLifecycleSignInstallError<'registry>",
        "repair: AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
        "repair: DurableAuthenticatedRecoveredWalValidateLifecycleRepair<'registry>",
        "installed: InstalledRecoveredWalSignRegistryCut<'registry>",
        "pub(crate) fn authenticate_final_wal_startup_authority(",
        "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
        "if !self.effects.is_empty()",
        "enum RecoveredWalStartupAuthorityV1",
        "PhaseVote(RecoveredWalVoteSign)",
        "ControlSign(RecoveredWalControlSign)",
        "ProductionLifecycleOwnerV1::open_storage_only_recovered_startup(",
        ".persist_repair()",
        ".install_recovered_sign()",
        ".open_production_owner_seals(",
        ".into_owner(registry, payload_store, body_store)",
        "ProductionLifecycleOwnerV1::from_recovered_wal_open(",
        "#[cfg(test)]\n    #[allow(clippy::result_large_err)]\n    fn finish_without_wal_vote(",
        "fn authenticate_recovered_parent_from_storage<'registry, 'body>(",
        "registry: &'registry mut LifecycleWorkRegistryHolder",
        "body_store: &'body mut super::v2_body_store::V2BodyStore",
        "registry.reconstruct_recovered_wal_validate_parent(",
        "fn authenticate_recovered_validate<'registry>(",
        "validate: RecoveredWalValidateRegistryCut<'registry>",
        "validate.join_recovered_vote(&verified, recovered_vote)",
        "parent_verification: self.adapter.parent_verification.clone()",
        "fn persist_repair_for_test(",
        "repair.persist_for_test(root)",
        "fn persist_reopened_repair_for_test(",
        "repair.persist_reopened_for_test(root)",
        "fn install_recovered_sign_for_test(",
        "repair.install_for_test(root)",
        "mut self,",
    ] {
        assert!(
            token.contains(required),
            "recovery token omitted {required}"
        );
    }
    assert_eq!(
        token
            .matches("ProductionRecoveredLifecycleOwnerAssemblyPermitV1::mint_paired()")
            .count(),
        1,
        "only the paired recovered startup may mint owner-assembly authority"
    );
    let cold_prepare = token
        .split_once("fn prepare_recovered_phase_vote_cold_adapter_stage")
        .expect("locate recovered phase-vote cold preparation")
        .1
        .split_once("fn install_recovered_phase_vote_sign_stage")
        .expect("locate the end of recovered phase-vote cold preparation")
        .0;
    assert!(cold_prepare.contains("context: adapter.wire_context.clone()"));
    assert!(cold_prepare.contains("proofs_of_possession: adapter.proofs_of_possession.clone()"));
    assert!(cold_prepare.contains("parent_verification: adapter.parent_verification.clone()"));
    assert!(
        cold_prepare
            .contains(".prepare_cold_adapter_startup(&verified, adapter_startup, body_store)")
    );
    let production_open = token
        .split_once("fn open_production_owner_seals(")
        .expect("locate paired production open")
        .1
        .split_once("#[cfg(test)]\nimpl<'registry> AuthenticatedRecoveredWalLifecycleStartup")
        .expect("locate the end of paired production open")
        .0;
    let open_signature = production_open
        .split_once(") -> Result<")
        .expect("paired production open signature ends")
        .0;
    assert!(!open_signature.contains("verified: VerifiedHeightContext"));
    assert!(production_open.contains("ProductionRecoveredLifecycleOwnerStartupV1"));
    assert!(!production_open.contains("Ok(("));
    for forbidden in [
        "#[derive(Clone)]\npub(crate) struct RecoveredWalVoteSign",
        "pub(crate) const fn frame_sequence(",
        "pub(crate) const fn persistence_id(",
        "pub(crate) const fn frame_hash(",
        "pub frame_sequence:",
        "pub persistence_id:",
        "pub frame_hash:",
        "pub tag:",
        "pub vote:",
        "pub prepare_certificate:",
        "impl Clone for RecoveredAdapterStartup",
        "FnOnce",
        "LifecycleCoordinator",
        "CandidateAdmission",
        "wal.append(",
        "pub(crate) fn into_parts(",
        "validate_effect: AdapterEffect",
        "validate_pending: PendingRuntimeEffectBinding",
        "RuntimeLifecycleOrdinalSource",
        "recovery: AuthenticatedLifecycleRecoveryCut",
        "recovered_vote: Option<RecoveredWalVoteSign>,\n        recovery:",
    ] {
        assert!(
            !token.contains(forbidden),
            "recovery token exposes forbidden surface {forbidden}"
        );
    }
    let recovered_vote_impl = token
        .split_once("impl RecoveredWalVoteSign {")
        .expect("locate the recovered WAL vote authority implementation")
        .1
        .split_once("\n}")
        .expect("locate the end of the recovered WAL vote authority implementation")
        .0;
    assert!(
        !recovered_vote_impl.contains("fn new("),
        "the recovered WAL vote authority must have no caller-visible constructor"
    );
    assert_eq!(
        production
            .matches("fn authenticate_recovered_wal_vote_sign(")
            .count(),
        1,
        "the sealed startup join owns one private recovery-token mint"
    );
    let frontier_start = production
        .find("fn authenticate_recovered_wal_frontier(")
        .expect("locate independent recovered WAL frontier authentication");
    let frontier_end = production[frontier_start..]
        .find("// RECOVERED_WAL_VOTE_SIGN_MINT_BEGIN")
        .map(|offset| frontier_start + offset)
        .expect("locate end of recovered WAL frontier authentication");
    let frontier = &production[frontier_start..frontier_end];
    for required in [
        "self.reducer.durable_state().last_id().get()",
        "self.wal.recovered_records().last()",
        "self.authenticate_recovered_wal_frame(frame)?",
        "envelope.persistence_id != durable_last_id",
    ] {
        assert!(
            frontier.contains(required),
            "recovered WAL frontier omitted {required}"
        );
    }
    let mint_start = production
        .find("// RECOVERED_WAL_VOTE_SIGN_MINT_BEGIN")
        .expect("locate recovered WAL vote-sign mint");
    let mint_end = production[mint_start..]
        .find("// RECOVERED_WAL_VOTE_SIGN_MINT_END")
        .map(|offset| mint_start + offset)
        .expect("locate end of recovered WAL vote-sign mint");
    let mint = &production[mint_start..mint_end];
    for required in [
        "startup_effects.len() > MAX_ADAPTER_EFFECTS_PER_MACRO_STEP",
        "self.reducer.awaiting_signature()",
        ".unsigned_vote_to_wire(*awaiting_vote)?",
        "vote.round != vote.proposal_round",
        "vote.proposal_round.context_id != self.wire_context.id()",
        "vote.proposal_round.height != self.wire_context.height",
        "self.wal.recovered_records().iter().rev()",
        "self.authenticate_recovered_wal_frame(frame)?",
        "WalRecordV2::PrepareIntent(candidate)",
        "WalRecordV2::LockAndCommit {",
        "vote: candidate",
        "prepare.round == prepare.proposal_round",
        "commit_intent_for_lock(locked)",
        "self.registry.reducer_qc_matches_wire(locked, &prepare)",
        "prepare.execution_commitment",
        "vote.execution_commitment",
        "RecoveredWalVoteReplayEvidenceV1::from_sealed_recovered_vote",
        "startup_effects.remove(effect_index)",
        "RecoveredWalVoteSign {",
    ] {
        assert!(mint.contains(required), "recovery mint omitted {required}");
    }
    for forbidden in [
        "FnOnce",
        "LifecycleCoordinator",
        "CandidateAdmission",
        "wal.append(",
        "drive_effects(",
        "step_reducer(",
        "publish_status(",
        "for_test(",
        "pub(crate) fn authenticate_recovered_wal_vote_sign(",
        "self.wal.recovered_records().last()",
    ] {
        assert!(
            !mint.contains(forbidden),
            "recovery mint invokes forbidden machinery {forbidden}"
        );
    }
    let runtime = include_str!("../v2_runtime.rs");
    let successor_start = runtime
        .find("pub(crate) struct RecoveredWalVoteSuccessor")
        .expect("locate recovered WAL successor");
    let successor_end = runtime[successor_start..]
        .find("fn pending_runtime_effect_binding_projection_hash")
        .map(|offset| successor_start + offset)
        .expect("locate end of recovered WAL successor surface");
    let successor = &runtime[successor_start..successor_end];
    for required in [
        "wal_identity: RecoveredWalFrameIdentity",
        "fn wal_identity_is_exact(&self) -> bool",
        "replay_evidence: RecoveredWalVoteReplayEvidenceV1",
        "fn replay_evidence_is_exact(&self) -> bool",
        "predecessor_effect: AdapterEffect",
        "predecessor_pending: PendingRuntimeEffectBinding",
        "pending: PendingRuntimeEffectBinding",
        "_prepare_certificate: Option<wire::QuorumCertificate>",
        "CommitSuccessorTagRelation::RecoveredMonotone",
        "fn into_ledger_lifecycle_projection(",
        "fn into_durable_lifecycle_projection(",
        "RecoveredWalCandidateProjectionPermit::new()",
        "_linearity: RecoveredWalCandidateProjectionLinearity",
    ] {
        assert!(
            successor.contains(required),
            "recovered successor omitted linear authority {required}"
        );
    }
    for forbidden in [
        "#[derive(Clone",
        "into_effect_and_pending",
        "into_parts",
        "fn predecessor_effect(",
        "fn predecessor_pending(",
        "pub(crate) const fn effect(",
        "pub(crate) const fn pending(",
    ] {
        assert!(
            !successor.contains(forbidden),
            "recovered successor exposes forbidden escape {forbidden}"
        );
    }
    let projection_start = runtime
        .find("pub(crate) fn project_recovered_wal_vote_successor(")
        .expect("locate recovered successor projection");
    let projection_end = runtime[projection_start..]
        .find("    fn project_recovered_ordinary_validate_commit_successor(")
        .map(|offset| projection_start + offset)
        .expect("locate end of recovered successor projection");
    let projection = &runtime[projection_start..projection_end];
    assert!(projection.contains("        self,"));
    assert!(projection.contains("Err((self, recovered))"));
    assert!(projection.contains("recovered.replay_evidence_is_exact()"));
    assert!(projection.contains("recovered.replay_evidence().clone()"));
    assert!(projection.contains("predecessor_pending: self"));
    assert!(projection.contains("recovered.prepare_certificate().and_then(|prepare|"));
    assert!(projection.contains("project_recovered_inherited_validate_commit_successor"));
    assert!(projection.contains("project_recovered_ordinary_validate_commit_successor"));
    let commit_relation_start = runtime
        .find("enum CommitSuccessorTagRelation")
        .expect("locate bounded Commit tag relation");
    let commit_relation_end = runtime[commit_relation_start..]
        .find("\nimpl PendingRuntimeEffectBinding")
        .map(|offset| commit_relation_start + offset)
        .expect("locate end of bounded Commit tag relation");
    let commit_relation = &runtime[commit_relation_start..commit_relation_end];
    for required in [
        "LiveExact",
        "RecoveredMonotone",
        "predecessor == successor",
        "successor.view() == vote_round.view",
        "predecessor.generation() == successor.generation()",
        "predecessor.view() >= vote_round.view",
        "predecessor.view() <= successor.view()",
        "recovered_prepare_matches_commit_vote",
        "prepare.execution_commitment == vote.execution_commitment",
    ] {
        assert!(
            commit_relation.contains(required),
            "bounded Commit tag relation omitted {required}"
        );
    }
    let live_commit = runtime
        .split_once("pub(crate) fn project_validate_sign_commit_successor(")
        .expect("locate live Commit projection")
        .1
        .split_once("/// Project a Commit-vote successor for an ordinary Validate")
        .expect("locate end of live Commit projection")
        .0;
    assert!(live_commit.contains("CommitSuccessorTagRelation::LiveExact"));
    let runtime_production = runtime
        .split_once("#[cfg(test)]\nmod tests")
        .expect("locate runtime test boundary")
        .0;
    assert_eq!(
        runtime_production
            .matches("project_recovered_inherited_validate_commit_successor(")
            .count(),
        2,
        "recovered inherited-Prepare retag is called only by its sealed projection"
    );
    assert_eq!(
        runtime_production
            .matches("project_recovered_ordinary_validate_commit_successor(")
            .count(),
        2,
        "recovered ordinary-Validate retag is called only by its sealed projection"
    );
    let replay = concat!(
        include_str!("../v2_lifecycle_replay_authority.rs"),
        include_str!("../v2_lifecycle_replay_authority_certified_body.rs")
    );
    for required in [
        "pub(crate) struct RecoveredWalVoteReplayEvidenceV1",
        "locator: PersistedWalFrameLocatorV1",
        "fn from_sealed_recovered_vote(",
        "fn exactly_matches_recovered_vote(",
        "fn project_recovered_vote_candidate(",
        "source.locator.exactly_matches_runtime(locator)",
        "LifecycleReplayAuthorityV1::decode_canonical",
        "wire::GlobalPhase::Prepare => tag.view() == vote.round.view",
        "wire::GlobalPhase::Commit => tag.view() >= vote.round.view",
        "wire::GlobalPhase::Prepare => self.tag.view == vote.round.view",
        "wire::GlobalPhase::Commit => true",
    ] {
        assert!(
            replay.contains(required),
            "recovered replay evidence omitted {required}"
        );
    }
    for forbidden in [
        "struct ReplayWalLocatorV1",
        "pub(crate) fn encoded(",
        "pub(crate) fn into_parts(",
        "pub(crate) fn locator(",
        "pub(crate) fn action(",
    ] {
        assert!(
            !replay.contains(forbidden),
            "recovered replay evidence exposes {forbidden}"
        );
    }
    for required in [
        "struct RecoveredDecisionApplyReplayLineageV1",
        "fetch: LifecycleReplayAuthorityV1",
        "body: RecoveredDecisionBodyPipelineReplayFamilyV1",
        "apply: LifecycleReplayAuthorityV1",
        "BodyPipelineOriginV1::RecoveredDecision",
        "WalReplayActionV1::FetchDecision",
        "WalReplayActionV1::ApplyDecision",
        "fn is_stage_closed(&self, context: LifecycleContext) -> bool",
        "DurableContinuationEdge::FetchToStore",
        "DurableContinuationEdge::StoreToValidate",
        "DurableContinuationEdge::ValidateToApply",
    ] {
        assert!(
            replay.contains(required),
            "recovered Decision body lineage omitted {required}"
        );
    }
    assert!(
        !replay.contains("project_exact_body_candidate"),
        "recovered Decision body lineage exposes a raw candidate projector"
    );
    let reconstructed_start = runtime
        .find("pub(crate) fn reconstruct_recovered_wal_vote_successor(")
        .expect("locate storage-authenticated runtime reconstruction");
    let reconstructed_end = runtime[reconstructed_start..]
        .find("\nimpl PendingRuntimeEffectBinding")
        .map(|offset| reconstructed_start + offset)
        .expect("locate end of storage-authenticated reconstruction");
    let reconstructed = &runtime[reconstructed_start..reconstructed_end];
    for required in [
        "parent.exactly_matches_recovered_vote(&recovered)",
        "parent.runtime_causal_lifecycle_key()",
        ".inherited_prepare_authority()",
        "project_recovered_wal_vote_successor(&predecessor, recovered)",
    ] {
        assert!(
            reconstructed.contains(required),
            "storage-authenticated runtime reconstruction omitted {required}"
        );
    }
    assert_eq!(
        reconstructed
            .matches(".inherited_prepare_authority()")
            .count(),
        2,
        "storage-authenticated reconstruction must bind both inherited Prepare fields"
    );
    for forbidden in [
        "RuntimeEffectOwnership",
        "RuntimeLifecycleOrdinalSource",
        "lifecycle_ordinal",
        "fresh_for_test",
        "into_parts",
        "pub(crate) fn effect(",
        "pub(crate) fn pending(",
    ] {
        assert!(
            !reconstructed.contains(forbidden),
            "storage-authenticated runtime reconstruction exposes {forbidden}"
        );
    }
    let body_store = include_str!("../v2_body_store.rs");
    let marker_start = body_store
        .find("pub(super) struct RecoveredValidatedBodyCut<'store>")
        .expect("locate recovered validated-body cut");
    let marker_end = body_store[marker_start..]
        .find("// DURABLE_BODY_VALIDATION_SURFACE_END")
        .map(|offset| marker_start + offset)
        .expect("locate end of recovered validated-body cut");
    let marker = &body_store[marker_start..marker_end];
    for required in [
        "store: &'store mut V2BodyStore",
        "impl Drop for RecoveredValidatedBodyCut<'_>",
        "self.store.validated.insert(self.key, validated)",
        "pub(super) fn exactly_matches_ledger_parent",
        "pub(super) fn into_validation_outcome",
        "pub(in crate::sumeragi) struct RecoveredDecisionApplyBodyCut<'store>",
        "store_identity: V2BodyStoreInstanceIdentity",
        "context: wire::HeightContext",
        "envelope: StoredBodyEnvelope",
        "impl Drop for RecoveredDecisionApplyBodyCut<'_>",
        "a detached recovered Decision body cannot collide while restoring",
    ] {
        assert!(
            marker.contains(required),
            "body marker cut omitted {required}"
        );
    }
    for forbidden in [
        "pub(crate) struct RecoveredValidatedBodyCut",
        "fn receipt(",
        "fn into_receipt(",
        "into_parts",
        "fn manifest(",
        "fn body(",
    ] {
        assert!(
            !marker.contains(forbidden),
            "body marker cut exposes forbidden surface {forbidden}"
        );
    }
    let recovered_parent_detach = body_store
        .split_once("pub(super) fn detach_recovered_validated_parent(")
        .expect("locate recovered validated-parent detach")
        .1
        .split_once("/// Execute one exact-body persistence task")
        .expect("locate end of recovered validated-parent detach")
        .0;
    for required in [
        "wire::GlobalPhase::Prepare => recovered.tag().view() == vote.round.view",
        "wire::GlobalPhase::Commit => recovered.tag().view() >= vote.round.view",
    ] {
        assert!(
            recovered_parent_detach.contains(required),
            "recovered validated-parent detach omitted {required}"
        );
    }
    assert!(!body_store.contains("#[derive(Clone)]\npub(super) struct RecoveredValidatedBodyCut"));
    for required in [
        "pub(crate) struct RevalidatedV2BodyStore(V2BodyStore)",
        "pub(crate) fn into_revalidated_startup(",
        "self.ensure_recovered_markers_revalidated()?",
        "fn matches_context(&self, context: &wire::HeightContext)",
        "fn detach_recovered_decision_apply_body(",
        "fn into_lifecycle_owner_store(",
        "if &self.0.context != expected_context",
    ] {
        assert!(
            body_store.contains(required),
            "revalidated same-store startup cut omitted {required}"
        );
    }
    assert!(!body_store.contains("impl Clone for RevalidatedV2BodyStore"));
    let decision_composite = body_store
        .split_once("struct RecoveredDecisionApplyAdapterPreviewV1<'store>")
        .expect("locate recovered Decision adapter composite")
        .1
        .split_once("/// One-shot call capability for deriving")
        .expect("locate end of recovered Decision adapter composite")
        .0;
    for required in [
        "_fetch: AuthenticatedRecoveredWalDecisionFetchProjection",
        "_body: RecoveredDecisionApplyBodyCut<'store>",
        "RecoveredDecisionApplyReplayLineageV1",
        "_staged: super::v2::RecoveredDecisionApplyStagedAdapterV1",
        "RecoveredDecisionApplyAdapterPreviewFailure<'store>",
    ] {
        assert!(
            decision_composite.contains(required),
            "recovered Decision adapter composite omitted {required}"
        );
    }
    for forbidden in [
        "fn effect(",
        "fn pending(",
        "fn candidate(",
        "fn body(",
        "fn replay(",
        "into_parts",
    ] {
        assert!(
            !decision_composite.contains(forbidden),
            "recovered Decision adapter composite exposes {forbidden}"
        );
    }
    let decision_wal_recovery = include_str!("../v2_lifecycle_wal_recovery.rs");
    let decision_preview = production
        .split_once("fn prepare_recovered_decision_apply_fast_forward(")
        .expect("locate recovered Decision adapter fast-forward")
        .1
        .split_once("/// Preview a certified Fetch completion directly")
        .expect("locate end of recovered Decision adapter fast-forward")
        .0;
    for required in [
        "let mut comparison_registry = self.registry.clone()",
        "prepare_direct_certified_body_available",
        "prepare_direct_body_stored",
        "prepare_direct_validation_succeeded",
        "apply_certificate == &certificate",
        "project_decision_apply_pending_lineage",
        "project_certified_fetch_store_successor",
        "project_store_validate_successor",
        "project_validate_apply_successor",
        "RecoveredDecisionApplyAdapterRollback",
        "reducer_fence_generation",
        "last_progress",
        "rollback.restore(&mut adapter)",
    ] {
        assert!(
            decision_preview.contains(required) || decision_wal_recovery.contains(required),
            "recovered Decision fast-forward omitted {required}"
        );
    }
    for forbidden in [
        ".commit()",
        "commit_recovered_decision_apply",
        "CandidateAdmission",
        "LifecycleCoordinator",
        "LifecycleWorkRegistry",
        ".wal.append(",
        "publish_status(",
    ] {
        assert!(
            !decision_preview.contains(forbidden),
            "recovered Decision fast-forward invokes {forbidden}"
        );
    }
    let ledger = reviewed_lifecycle_ledger_source_for_test();
    let parent_start = ledger
        .find("pub(crate) struct AuthenticatedRecoveredWalValidateLedgerParent")
        .expect("locate opaque recovered ledger parent");
    let parent_end = ledger[parent_start..]
        .find("    /// Purely stage one adapter-authenticated WAL-ahead")
        .map(|offset| parent_start + offset)
        .expect("locate end of recovered ledger parent factory");
    let parent = &ledger[parent_start..parent_end];
    for required in [
        "fn authenticate_recovered_wal_validate_parent(",
        "recovered.prepare_certificate().is_none()",
        "prepare.execution_commitment == vote.execution_commitment",
        "wire::GlobalPhase::Prepare => recovered.tag().view() == vote.round.view",
        "wire::GlobalPhase::Commit => recovered.tag().view() >= vote.round.view",
        "LifecycleStageKind::ValidateBody",
        "DurableContinuation::None",
        "TerminalOutcome::Advanced",
        "pub(crate) fn matches_durable_receipt(",
        "pub(in crate::sumeragi) fn project_recovered_candidate(",
        "Hash::prehashed(*self.owner.causal_root().digest().as_bytes())",
    ] {
        assert!(
            parent.contains(required),
            "ledger parent seal omitted {required}"
        );
    }
    for forbidden in [
        "into_parts",
        "pub(crate) fn candidate(",
        "pub(crate) fn ordinal(",
        "RuntimeEffectOwnership",
        "RuntimeLifecycleOrdinalSource",
    ] {
        assert!(
            !parent.contains(forbidden),
            "ledger parent seal exposes forbidden surface {forbidden}"
        );
    }
    let registry = reviewed_lifecycle_work_registry_source_for_test();
    let exact_store = registry
        .split_once("pub(crate) struct OpenedRecoveredWalValidateLedger")
        .expect("locate exact recovered-WAL store cut")
        .1
        .split_once(
            "/// Opaque failure from storage-authenticated recovered-parent reconstruction.",
        )
        .expect("locate end of exact recovered-WAL store transaction")
        .0;
    for required in [
        "store: super::ledger::LifecycleLedgerStoreV1",
        "opened: super::ledger::LifecycleLedgerV1",
        "struct PersistedRecoveredWalValidateLedger<'registry>",
        "struct InstalledRecoveredWalSignStorage<'registry>",
        "persist_recovered_wal_repair",
        "install_recovered_wal_sign",
        "authenticate_durable_certified_fetch_startup(verified, body_store)",
        "assemble_storage_only_with_recovered_wal_sign_and_durable_fetch_startup",
        "install_alongside_recovered_wal_authority",
        "open_with_exact_store_authority(authority, store, payload_store, recovery)",
        "let body_store_identity = body_store.instance_identity()",
        "let payload_store_identity = payload_store.instance_identity()",
        "ProductionOpenedRecoveredWalSignLifecycleCut",
    ] {
        assert!(
            exact_store.contains(required),
            "exact recovered-WAL store transaction omitted {required}"
        );
    }
    for forbidden in [
        "ledger_root",
        "LifecycleLedgerStoreV1::open(",
        "fn into_parts(",
        "pub(crate) fn store(",
        "pub(crate) fn ledger(",
    ] {
        assert!(
            !exact_store.contains(forbidden),
            "exact recovered-WAL store transaction exposed {forbidden}"
        );
    }
    let owner_seal = registry
        .split_once("pub(crate) struct RecoveredWalProductionOwnerOpenV1")
        .expect("locate recovered-WAL owner seal")
        .1
        .split_once("/// Opaque fail-stop coordinator-open error")
        .expect("locate end of recovered-WAL owner seal")
        .0;
    for required in [
        "verified: VerifiedHeightContext",
        "registry_identity: ConcreteLifecycleWorkRegistryInstanceIdentity",
        "V2BodyStoreInstanceIdentity",
        "CertifiedServePayloadStoreInstanceIdentity",
    ] {
        assert!(
            owner_seal.contains(required),
            "recovered-WAL owner seal omitted {required}"
        );
    }
    let authenticated_open = registry
        .split_once("pub(crate) struct ProductionOpenedRecoveredWalSignLifecycleCut")
        .expect("locate store-authenticated recovered open")
        .1
        .split_once("/// No-lifetime exact-open seal")
        .expect("locate the end of the store-authenticated recovered open")
        .0;
    for required in [
        "opened: OpenedRecoveredWalSignLifecycleCut<'registry>",
        "verified: VerifiedHeightContext",
        "V2BodyStoreInstanceIdentity",
        "CertifiedServePayloadStoreInstanceIdentity",
    ] {
        assert!(
            authenticated_open.contains(required),
            "store-authenticated recovered open omitted {required}"
        );
    }
    let owner_conversion = registry
        .split_once("impl ProductionOpenedRecoveredWalSignLifecycleCut<'_>")
        .expect("locate store-authenticated owner conversion")
        .1
        .split_once("#[cfg(test)]\nimpl OpenedRecoveredWalSignLifecycleCut")
        .expect("locate the end of store-authenticated owner conversion")
        .0;
    assert!(owner_conversion.contains("fn into_production_owner_open(\n        self,"));
    for forbidden in [
        "verified: VerifiedHeightContext,",
        "body_store_identity:",
        "payload_store_identity:",
    ] {
        let signature = owner_conversion
            .split_once("fn into_production_owner_open(")
            .expect("locate owner conversion signature")
            .1
            .split_once(") ->")
            .expect("owner conversion signature ends")
            .0;
        assert!(
            !signature.contains(forbidden),
            "owner conversion accepts caller-supplied authority {forbidden}"
        );
    }
    let owner_assembly = ledger
        .split_once("pub(in crate::sumeragi) fn from_recovered_wal_open(")
        .expect("locate the recovered owner assembly")
        .1
        .split_once("#[cfg(test)]\nimpl ProductionLifecycleOwnerV1")
        .expect("locate the end of recovered owner assembly")
        .0;
    assert!(owner_assembly.contains("_permit: ProductionRecoveredLifecycleOwnerAssemblyPermitV1"));
    let wal_recovery = include_str!("../v2_lifecycle_wal_recovery.rs");
    for required in [
        "struct AuthenticatedRecoveredWalVoteProjection",
        "_permit: RecoveredWalCandidateProjectionPermit",
        "RecoveredValidatePayloadAuthority::Ledger(parent)",
        "RecoveredValidatePayloadAuthority::Durable",
        "successor.into_ledger_lifecycle_projection(verified, parent)",
        "successor.into_durable_lifecycle_projection(verified, receipt, replay_evidence)",
    ] {
        assert!(
            wal_recovery.contains(required),
            "recovered WAL lifecycle join omitted {required}"
        );
    }
    for forbidden in [
        "projection::admission_request(",
        "bind_projected_candidate(",
        "predecessor_effect()",
        "predecessor_pending()",
        "successor.pending()",
    ] {
        assert!(
            !wal_recovery.contains(forbidden),
            "recovered WAL lifecycle join exposed raw projection surface {forbidden}"
        );
    }
    let recovered_projection_start = wal_recovery
        .find("impl AuthenticatedRecoveredWalVoteProjection")
        .expect("locate recovered WAL projection implementation");
    let recovered_projection_end = wal_recovery[recovered_projection_start..]
        .find("#[cfg_attr(not(test), allow(dead_code))]")
        .map(|offset| recovered_projection_start + offset)
        .expect("locate end of recovered WAL projection implementation");
    let recovered_projection = &wal_recovery[recovered_projection_start..recovered_projection_end];
    for forbidden in [
        "pub(super) const fn parent(&self)",
        "pub(super) const fn child(&self)",
        "pub(in crate::sumeragi) const fn parent(&self)",
        "pub(in crate::sumeragi) const fn child(&self)",
    ] {
        assert!(
            !recovered_projection.contains(forbidden),
            "recovered WAL projection exposes candidate oracle {forbidden}"
        );
    }
}
