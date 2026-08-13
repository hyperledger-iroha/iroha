    #[test]
    fn recovered_wal_sign_open_is_opaque_precommit_checked_and_runner_inert() {
        let source = reviewed_lifecycle_work_registry_source_for_test();
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let open = production
            .split_once("// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_BEGIN")
            .expect("recovered Sign coordinator open begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_COORDINATOR_OPEN_END")
            .expect("recovered Sign coordinator open ends")
            .0;
        for required in [
            "pub(super) struct AuthenticatedRecoveredWalSignProjection",
            "parent: CandidateAdmission",
            "child: CandidateAdmission",
            "parent_address: ConcreteWorkAddress",
            "child_address: ConcreteWorkAddress",
            "fn repaired_pair_is_exact(",
            "record.replay_matches_candidate(&self.child)",
            "parent.replay_matches_candidate(&self.parent)",
            "parent.terminal() == Some(Some(super::TerminalOutcome::Advanced))",
            "parent.continuation()",
            "fn insert_repaired_child_from_record(",
            "record.owner() != self.child_address.owner",
            "record.ordinal() != self.child_address.ordinal",
            "fn splice_candidates(",
            "(Some(parent), None) if parent == &self.parent",
            "(None, Some(child)) if child == &self.child",
            "pub(crate) struct OpenedRecoveredWalSignLifecycleCut<'registry>",
            "pub(crate) struct RecoveredWalSignLifecycleOpenError<'registry>",
            "LifecycleCoordinator::prepare_with_authority_borrowed(",
            "self.prepared_join_is_exact(&prepared, &recovery, &projection)",
            "prepared.commit(payload_store, &recovery)",
            "self.opened_join_is_exact(&coordinator, &recovery, &projection)",
            "PostCommitMismatch",
        ] {
            assert!(
                open.contains(required),
                "recovered Sign open omitted {required}"
            );
        }
        for forbidden in [
            "pub parent:",
            "pub child:",
            "fn new(",
            "into_parts",
            "pub(crate) fn effect(",
            "pub(crate) fn pending(",
            "pub(crate) fn receipt(",
            "publish_status(",
            "RuntimeEffectOwnership",
        ] {
            assert!(
                !open.contains(forbidden),
                "recovered Sign open exposed forbidden surface {forbidden}"
            );
        }
        let precommit = open
            .find("self.prepared_join_is_exact(&prepared, &recovery, &projection)")
            .expect("precommit exact join exists");
        let commit = open
            .find("prepared.commit(payload_store, &recovery)")
            .expect("durable open commit exists");
        let postcommit = open
            .find("self.opened_join_is_exact(&coordinator, &recovery, &projection)")
            .expect("postcommit exact join exists");
        assert!(precommit < commit && commit < postcommit);

        for seed in [
            "seed_parent_candidate_for_test",
            "seed_child_candidate_for_test",
            "seed_both_candidates_for_test",
            "seed_parent_recovery_for_test",
            "seed_child_recovery_for_test",
            "seed_both_recovery_for_test",
        ] {
            let offset = open.find(seed).unwrap_or_else(|| panic!("missing {seed}"));
            let prefix = &open[offset.saturating_sub(180)..offset];
            assert!(
                prefix.contains("#[cfg(test)]"),
                "fixture seed {seed} must remain test-only"
            );
        }
        let projection_impl = open
            .split_once("impl AuthenticatedRecoveredWalSignProjection")
            .expect("opaque installed projection impl exists")
            .1
            .split_once("/// Sealed coordinator-open result")
            .expect("opaque installed projection impl ends")
            .0;
        for seed in [
            "seed_parent_candidate_for_test",
            "seed_child_candidate_for_test",
            "seed_both_candidates_for_test",
        ] {
            assert!(
                projection_impl.contains(seed),
                "fixture seed {seed} must require the opaque installed projection"
            );
        }
        for seed in [
            "seed_parent_recovery_for_test",
            "seed_child_recovery_for_test",
            "seed_both_recovery_for_test",
        ] {
            let offset = open.find(seed).unwrap_or_else(|| panic!("missing {seed}"));
            let method = &open[offset
                ..offset
                    + open[offset..]
                        .find("\n    }\n")
                        .unwrap_or_else(|| panic!("fixture seed {seed} has no method end"))];
            assert!(
                method.contains("self.authenticated_projection()"),
                "fixture seed {seed} must mint its opaque projection from the installed cut"
            );
            let signature = method
                .split_once('{')
                .expect("fixture seed has a function body")
                .0;
            assert!(
                !signature.contains("AuthenticatedRecoveredWalSignProjection"),
                "fixture seed {seed} must not accept a caller-supplied projection"
            );
        }

        let open_source = include_str!("../v2_lifecycle_open.rs");
        let splice = open_source
            .split_once("// RECOVERED_WAL_SIGN_RECOVERY_SPLICE_BEGIN")
            .expect("opaque recovery splice begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_RECOVERY_SPLICE_END")
            .expect("opaque recovery splice ends")
            .0;
        assert!(splice.contains("projection: &AuthenticatedRecoveredWalSignProjection"));
        for forbidden in [
            "parent: &CandidateAdmission",
            "child: &CandidateAdmission",
            "CandidateAdmission) ->",
            "into_parts",
        ] {
            assert!(
                !splice.contains(forbidden),
                "recovery splice accepts forbidden caller material {forbidden}"
            );
        }
        let borrowed = open_source
            .split_once("// RECOVERED_WAL_SIGN_BORROWED_OPEN_BEGIN")
            .expect("borrowed recovery open begins")
            .1
            .split_once("// RECOVERED_WAL_SIGN_BORROWED_OPEN_END")
            .expect("borrowed recovery open ends")
            .0;
        assert!(borrowed.contains("prepare_with_authority_borrowed("));
        assert!(borrowed.contains("PreparedLifecycleCoordinatorOpen"));

        for runner_source in [
            include_str!("../v2_runner.rs"),
            include_str!("../v2_worker.rs"),
            include_str!("../v2_effects.rs"),
        ] {
            assert!(!runner_source.contains("open_coordinator_from_verified"));
            assert!(!runner_source.contains("OpenedRecoveredWalSignLifecycleCut"));
        }
    }
    #[test]
    fn durable_validate_async_handoff_surface_is_move_only_scheduler_free_and_inert() {
        let source = reviewed_lifecycle_work_registry_source_for_test();
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN")
            .expect("detached Validate declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_END")
            .expect("detached Validate declarations end")
            .0;
        for required in [
            "struct DetachedDurableValidateExecution",
            "address: ConcreteWorkAddress",
            "incumbent_digest: LifecycleDigest",
            "tag: EventTag",
            "round: wire::ConsensusRound",
            "subject: wire::BlockSubject",
            "durable_receipt: DurableBodyReceipt",
            "expected_manifest_hash: HashOf<wire::PayloadManifest>",
            "causal_lifecycle_key: Hash",
            "candidate_statement: Option<RuntimeCandidateSemanticStatement>",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct ExecutedDurableValidateExecution",
            "request: DetachedDurableValidateExecution",
            "outcome: DurableBodyValidationOutcome",
            "struct PreparedDurableValidateCompletion<'a>",
            "&'a mut ConcreteLifecycleWorkRegistry",
        ] {
            assert!(
                declarations.contains(required),
                "detached Validate declarations omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "detached Validate declarations acquired forbidden scheduler surface: {forbidden}"
            );
        }

        let implementation = production
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN")
            .expect("detached Validate implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_END")
            .expect("detached Validate implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 0);
        assert_eq!(implementation.matches("fn execute").count(), 1);
        assert_eq!(
            implementation
                .matches("execute_durable_validation(")
                .count(),
            1
        );
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
            "ordinal:",
            "TurnLease",
            "WaitToken",
            "ReadyEvent",
            "SchedulerInputs",
            "SchedulerRank",
            "TurnPlan",
            "TurnOutcome",
            "into_parts",
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "detached Validate implementation acquired forbidden authority: {forbidden}"
            );
        }

        let reattachment = production
            .split("pub(super) fn reattach_durable_validate_execution(")
            .nth(1)
            .expect("detached Validate has one reattachment method")
            .split("pub(super) fn borrow_for_lease(")
            .next()
            .expect("generic borrow follows detached Validate reattachment");
        for required in [
            "ConcreteWorkAddress::new",
            "work.validates_at(request.address)",
            "work.digest != request.incumbent_digest",
            "DurableValidateBody(validate)",
            "exactly_binds_adapter_effect",
            "causal_lifecycle_key() != &request.causal_lifecycle_key",
            "candidate_statement() != request.candidate_statement",
            "executed.outcome.durable_body() != &request.durable_receipt",
            "validate_validated_receipt_authority(validate, receipt)?",
            "return Err((error, executed))",
        ] {
            assert!(
                reattachment.contains(required),
                "detached Validate reattachment omitted {required}"
            );
        }
        for forbidden in [
            "fn commit(",
            ".insert(",
            ".remove(",
            "enqueue_",
            ".publish_ready(",
            ".replace_before_publication(",
        ] {
            assert!(
                !reattachment.contains(forbidden),
                "detached Validate reattachment acquired forbidden mutation: {forbidden}"
            );
        }

        assert_eq!(production.matches("pub(super) fn detach(").count(), 1);
        assert_eq!(
            production
                .matches("pub(super) fn reattach_durable_validate_execution(")
                .count(),
            1
        );
        assert_eq!(production.matches(".detach()").count(), 1);
        for caller_source in [
            include_str!("../v2_lifecycle_selector.rs"),
            include_str!("../v2_lifecycle_coordinator.rs"),
            include_str!("../v2_effects.rs"),
            include_str!("../v2_worker.rs"),
            include_str!("../v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("DetachedDurableValidateExecution"));
            assert!(!caller_source.contains("reattach_durable_validate_execution"));
        }
    }

    #[test]
    fn durable_validate_wait_dispatch_is_move_only_single_entry_and_unwired() {
        let registry_source = reviewed_lifecycle_work_registry_source_for_test();
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");
        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_BEGIN")
            .expect("wait-dispatch declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_END")
            .expect("wait-dispatch declarations end")
            .0;
        for required in [
            "struct DurableValidateWakeAuthority",
            "wait_token: WaitToken",
            "struct DurableValidateDispatch",
            "request: DetachedDurableValidateExecution",
            "struct ExecutedDurableValidateDispatch",
            "executed: ExecutedDurableValidateExecution",
        ] {
            assert!(
                declarations.contains(required),
                "wait-dispatch declaration omitted {required}"
            );
        }
        for forbidden in [
            "derive(Clone",
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "lifecycle_ordinal",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "wait-dispatch declaration acquired legacy authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_BEGIN")
            .expect("wait-dispatch implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_END")
            .expect("wait-dispatch implementation ends")
            .0;
        assert_eq!(implementation.matches("pub(super) fn execute").count(), 1);
        assert!(implementation.contains("request.execute(body_store, validator)"));
        assert!(implementation.contains("Err((error, Self { request, wake }))"));
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "fn commit(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "wait-dispatch execution acquired forbidden authority: {forbidden}"
            );
        }
        assert_eq!(
            registry_production.matches("pub(super) fn execute").count(),
            1,
            "the outer dispatch must be the sole externally visible validation execution path"
        );
        assert_eq!(
            registry_production
                .matches("projection::durable_validation_wait_source(")
                .count(),
            1,
            "only the sealed registry preflight may call the raw wait projection"
        );

        let concrete_source = include_str!("../v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn begin_durable_validate_dispatch(")
                .count(),
            1
        );
        let entrypoint = concrete_production
            .split("pub(super) fn begin_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one dispatch entrypoint")
            .split("/// Atomically publish one exact executable Validate result across the")
            .next()
            .expect("Validate completion follows dispatch entrypoint");
        for required in [
            "claimed_durable_validate_record_is_exact",
            "prepare_durable_validate_execution",
            "prepared.matches_durable_payload(metadata.payload)",
            "durable_validation_wait_source",
            "observed_generation",
            "observed_generation == u64::MAX",
            "AliasedWaitSource",
            "stage_durable_transaction",
            "TurnOutcome::Blocked(wait_token)",
            "staged_durable_validate_wait_is_exact",
            "seal_waiting_dispatch(wait_token)",
            "DurableValidateDispatchError, TurnLease",
            "*self = next",
        ] {
            assert!(
                entrypoint.contains(required),
                "dispatch entrypoint omitted {required}"
            );
        }
        let staging = entrypoint
            .find("stage_durable_transaction")
            .expect("entrypoint stages coordinator state");
        let sealing = entrypoint
            .find("seal_waiting_dispatch")
            .expect("entrypoint seals its dispatch");
        let publication = entrypoint
            .find("*self = next")
            .expect("entrypoint publishes its staged coordinator");
        assert!(staging < sealing && sealing < publication);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "enqueue_",
            "publish_ready",
            "ReadyEvent",
            "replace_before_publication",
            "persist_durable_projection",
            "checked_add(",
            "LeaseId(",
            "SchedulerRank::new",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "dispatch entrypoint acquired forbidden authority: {forbidden}"
            );
        }

        let claimed_helper = concrete_production
            .split("fn claimed_durable_validate_record_is_exact(")
            .nth(1)
            .expect("claimed Validate exactness helper exists")
            .split("fn staged_durable_validate_wait_is_exact(")
            .next()
            .expect("staged wait helper follows claimed exactness");
        for required in [
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
            "durable_validate_payload_is_exact(record.key, metadata.payload)",
        ] {
            assert!(
                claimed_helper.contains(required),
                "claimed Validate exactness omitted reverse identity check {required}"
            );
        }
        let staged_helper = concrete_production
            .split("fn staged_durable_validate_wait_is_exact(")
            .nth(1)
            .expect("staged Validate wait helper exists")
            .split("fn concrete_work_location(")
            .next()
            .expect("concrete location helper follows staged wait");
        for required in [
            "next.episode_authority == current.episode_authority",
            "next.ledger_store.is_some() == current.ledger_store.is_some()",
            "next.active_lease.is_none()",
            "next.observed_generation == expected_observed",
        ] {
            assert!(
                staged_helper.contains(required),
                "staged Validate wait omitted exact projection check {required}"
            );
        }

        let projection_source = include_str!("../v2_lifecycle_projection.rs");
        let projection = projection_source
            .split("pub(super) fn durable_validation_wait_source(")
            .nth(1)
            .expect("durable validation wait projection exists")
            .split("pub(super) fn reducer_fence_wait_source")
            .next()
            .expect("reducer-fence projection follows durable validation");
        for required in [
            "DURABLE_VALIDATION_WAIT_SOURCE_DOMAIN",
            "owner.causal_root().digest()",
            "owner.first_admission_ordinal()",
            "incumbent_digest",
            "causal_lifecycle_key",
            "candidate_statement",
            "durable_frame_hash",
            "expected_manifest_hash",
            "lifecycle_key",
            "lifecycle_stage",
        ] {
            assert!(
                projection.contains(required),
                "durable validation wait projection omitted {required}"
            );
        }

        for caller_source in [
            include_str!("../v2_lifecycle_selector.rs"),
            include_str!("../v2_lifecycle_coordinator.rs"),
            include_str!("../v2_effects.rs"),
            include_str!("../v2_worker.rs"),
            include_str!("../v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("begin_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateDispatch"));
        }
    }

    #[test]
    fn durable_validate_volatile_completion_is_atomic_move_only_and_unwired() {
        let registry_source = reviewed_lifecycle_work_registry_source_for_test();
        let registry_production = registry_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let carrier = registry_production
            .split("struct DurableValidateCompletion {")
            .nth(1)
            .expect("Validate completion carrier has one declaration")
            .split("enum ConcreteLifecycleWorkKind")
            .next()
            .expect("work-kind inventory follows Validate completion carrier");
        for required in [
            "address: ConcreteWorkAddress",
            "incumbent: DurableValidateBody",
            "incumbent_digest: LifecycleDigest",
            "outcome: DurableBodyValidationOutcome",
            "self.incumbent.validates(self.incumbent_digest)",
            "self.address.owner.causal_root()",
            "exactly_binds_adapter_effect",
            "self.outcome.durable_body() == &self.incumbent.durable_receipt",
            "self.incumbent.durable_receipt.manifest_hash()",
            "self.incumbent.expected_manifest_hash",
            "validate_validated_receipt_authority(&self.incumbent, receipt)",
            "durable_validate_completion_digest(",
            "installed_digest != self.incumbent_digest",
        ] {
            assert!(
                carrier.contains(required),
                "Validate completion carrier omitted {required}"
            );
        }
        for forbidden in ["derive(Clone", "fn new(", "into_parts"] {
            assert!(
                !carrier.contains(forbidden),
                "Validate completion carrier acquired raw or remintable authority: {forbidden}"
            );
        }

        let rejected_digest = registry_production
            .split("fn rejected_body_completion_digest(")
            .nth(1)
            .expect("rejected completion has one digest helper")
            .split("fn durable_validate_outcome_kind(")
            .next()
            .expect("outcome classification follows rejected digest");
        assert!(rejected_digest.contains("identity.canonical_code()"));
        assert!(!rejected_digest.contains("reason"));
        let validated_authority = registry_production
            .split("fn validate_validated_receipt_authority(")
            .nth(1)
            .expect("validated receipt has one shared authority helper")
            .split("fn validated_body_completion_digest(")
            .next()
            .expect("validated digest follows shared authority helper");
        for required in [
            "validated_receipt.durable() != &validate.durable_receipt",
            "validated_receipt.execution_commitment().validate().is_err()",
            "validate.pending.candidate_statement()",
            "statement.context_id() != round.context_id",
            "statement.proposal_round() != *round",
            "statement.subject() != Some(*subject)",
            ".execution_commitment()",
            "DurableValidateExecutionError::ConflictingValidationCommitment",
        ] {
            assert!(
                validated_authority.contains(required),
                "shared validated authority helper omitted {required}"
            );
        }
        assert_eq!(
            registry_production
                .matches("validate_validated_receipt_authority(")
                .count(),
            8,
            "carrier validation, classification, binding, reattachment, Ready preflight, recovery, and fixed adapter join must share one helper"
        );

        let declarations = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_BEGIN")
            .expect("volatile completion declarations begin")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_END")
            .expect("volatile completion declarations end")
            .0;
        for required in [
            "struct DurableValidateCompletionAuthority",
            "lifecycle_key: LifecycleKey",
            "lifecycle_stage: LifecycleStage",
            "struct PublishedValidated",
            "struct PublishedRejected",
            "struct DeferredDurableValidateDispatch",
            "dispatch: ExecutedDurableValidateDispatch",
            "enum DurableValidateCompletionPublication",
            "#[allow(variant_size_differences, clippy::large_enum_variant)]",
            "struct PreparedExecutedDurableValidateCompletion<'a>",
            "struct StagedDurableValidateCompletion<'a>",
            "request: Option<DetachedDurableValidateExecution>",
            "wake: Option<DurableValidateWakeAuthority>",
        ] {
            assert!(
                declarations.contains(required),
                "volatile completion declarations omitted {required}"
            );
        }
        for move_only in [
            "pub(super) struct DeferredDurableValidateDispatch",
            "pub(super) struct PreparedExecutedDurableValidateCompletion<'a>",
            "pub(super) struct StagedDurableValidateCompletion<'a>",
        ] {
            let declaration = declarations
                .split(move_only)
                .next()
                .expect("move-only declaration prefix exists")
                .rsplit("#[derive(")
                .next()
                .expect("derive prefix is inspectable");
            assert!(
                !declaration.contains("Clone"),
                "{move_only} must remain move-only"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeEffectOwnership",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "TurnPlan",
        ] {
            assert!(
                !declarations.contains(forbidden),
                "volatile completion declarations acquired legacy scheduler authority: {forbidden}"
            );
        }

        let implementation = registry_production
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_BEGIN")
            .expect("volatile completion implementation begins")
            .1
            .split_once("// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_END")
            .expect("volatile completion implementation ends")
            .0;
        for required in [
            "pub(super) fn stage_executable_carrier",
            "ConcreteLifecycleWorkKind::DurableValidateBody(incumbent)",
            "ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)",
            "impl Drop for StagedDurableValidateCompletion<'_>",
            "drop(self.restore())",
            "pub(super) fn missing_reference",
        ] {
            assert!(
                implementation.contains(required),
                "volatile completion implementation omitted {required}"
            );
        }
        assert_eq!(implementation.matches("pub(super) fn commit(").count(), 1);
        let commit = implementation
            .split("pub(super) fn commit(mut self)")
            .nth(1)
            .expect("staged completion has one infallible commit")
            .split("impl Drop for StagedDurableValidateCompletion")
            .next()
            .expect("guard Drop follows commit");
        assert!(commit.contains("self.armed = false;"));
        assert!(commit.contains("self.publication"));
        for forbidden in [
            ".get(", ".insert(", ".remove(", "expect(", "assert", "panic!", "?;", "Result<",
        ] {
            assert!(
                !commit.contains(forbidden),
                "post-swap guard commit acquired a fallible operation: {forbidden}"
            );
        }
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "replace_before_publication",
            "enqueue_",
            "persist_durable_projection",
            "into_parts",
            "pub(super) fn new(",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "volatile completion implementation acquired forbidden authority: {forbidden}"
            );
        }

        let concrete_source = include_str!("../v2_lifecycle_concrete_admission.rs");
        let concrete_production = concrete_source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("concrete admission has one production prefix");
        assert_eq!(
            concrete_production
                .matches("pub(super) fn complete_durable_validate_dispatch(")
                .count(),
            1,
            "there must be one sealed coordinator completion entrypoint"
        );
        assert_eq!(
            concrete_production
                .matches("prepare_executed_durable_validate_completion(dispatch)")
                .count(),
            1,
            "only the coordinator entrypoint may reattach a full dispatch"
        );
        let entrypoint = concrete_production
            .split("pub(super) fn complete_durable_validate_dispatch(")
            .nth(1)
            .expect("concrete admission has one completion entrypoint")
            .split("/// Atomically admit and register one exact adapter effect.")
            .next()
            .expect("generic admission follows completion entrypoint");
        for required in [
            "prepare_executed_durable_validate_completion(dispatch)",
            "waiting_durable_validate_record_is_exact",
            "prepared.defer_merge_sidecar()",
            "authority.ready_event()",
            "stage_durable_transaction()",
            "publish_ready(ready_event)",
            "staged_durable_validate_ready_is_exact",
            "prepared.stage_executable_carrier()?",
            "core::mem::swap(self, &mut next);\n        let published = staged_registry.commit();",
        ] {
            assert!(
                entrypoint.contains(required),
                "completion entrypoint omitted {required}"
            );
        }
        let coordinator_stage = entrypoint
            .find("stage_durable_transaction()")
            .expect("completion stages a coordinator copy");
        let registry_stage = entrypoint
            .find("prepared.stage_executable_carrier()?")
            .expect("completion stages the exact registry carrier");
        let coordinator_swap = entrypoint
            .find("core::mem::swap(self, &mut next)")
            .expect("completion swaps the checked coordinator copy");
        let registry_commit = entrypoint
            .find("staged_registry.commit()")
            .expect("completion infallibly disarms the registry guard");
        assert!(coordinator_stage < registry_stage);
        assert!(registry_stage < coordinator_swap);
        assert!(coordinator_swap < registry_commit);
        for forbidden in [
            "EffectWorkId",
            "BodyValidationTask",
            "RuntimeLifecycleOrdinalSource",
            "SchedulerRank",
            "LeaseId(",
            "next_lease",
            "enqueue_",
            "persist_durable_projection",
            "ledger_store.",
            "replace_before_publication",
        ] {
            assert!(
                !entrypoint.contains(forbidden),
                "completion entrypoint acquired forbidden durable or scheduler machinery: {forbidden}"
            );
        }

        let waiting_exact = concrete_production
            .split("fn waiting_durable_validate_record_is_exact(")
            .nth(1)
            .expect("waiting Validate exactness helper exists")
            .split("fn staged_durable_validate_ready_is_exact(")
            .next()
            .expect("staged Ready helper follows waiting exactness");
        for required in [
            "record.key == authority.lifecycle_key()",
            "record.stage == authority.lifecycle_stage()",
            "record.episode.frozen_predecessors.is_empty()",
            "episode_authority.universe_for(record.key)",
            "episode_authority.admits_slots(",
            "filter(|candidate| candidate.ordinal == record.ordinal)",
            "filter(|candidate| candidate.key == record.key)",
            "filter(|ordinal| **ordinal == record.ordinal)",
            "filter(|owner| **owner == record.owner)",
            "durable_validate_payload_is_exact(record.key, metadata.payload)",
            "authority.matches_durable_payload(metadata.payload)",
        ] {
            assert!(
                waiting_exact.contains(required),
                "waiting completion exactness omitted {required}"
            );
        }

        for caller_source in [
            include_str!("../v2.rs"),
            include_str!("../v2_lifecycle_selector.rs"),
            include_str!("../v2_lifecycle_coordinator.rs"),
            include_str!("../v2_effects.rs"),
            include_str!("../v2_worker.rs"),
            include_str!("../v2_runner.rs"),
        ] {
            assert!(!caller_source.contains("complete_durable_validate_dispatch"));
            assert!(!caller_source.contains("DurableValidateCompletionPublication"));
        }
    }

    #[test]
    fn certified_fetch_dequeue_commit_requires_the_durable_token() {
        let source = reviewed_lifecycle_work_registry_source_for_test();
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let preflight_declaration = production
            .split("pub(super) struct PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one declaration")
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .next()
            .expect("durable token follows selector preflight");
        assert!(
            preflight_declaration.contains("replay_origin: AuthenticatedCertifiedFetchReplayOriginV1")
        );
        assert!(!preflight_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(!preflight_declaration.contains("derive(Clone"));

        let durable_declaration = production
            .split("pub(super) struct PreparedDurableCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("durable completion token has one declaration")
            .split("pub(super) enum RegistryPublicationError")
            .next()
            .expect("registry publication error follows durable token");
        assert!(durable_declaration.contains("DurableCertifiedFetchBodyReceipt"));
        assert!(durable_declaration.contains("replay_evidence: CertifiedFetchReplayEvidenceV1"));
        assert!(!durable_declaration.contains("derive(Clone"));

        let preflight_impl = production
            .split("impl<'a> PreparedCertifiedFetchCompletion<'a>")
            .nth(1)
            .expect("selector preflight has one implementation")
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .next()
            .expect("durable implementation follows selector preflight");
        assert!(preflight_impl.contains("pub(super) fn bind_durable_body_receipt"));
        assert!(!preflight_impl.contains("fn commit_after_exact_dequeue("));
        assert!(!preflight_impl.contains(".remove("));
        assert!(!preflight_impl.contains(".insert("));

        let durable_impl = production
            .split("impl PreparedDurableCertifiedFetchCompletion<'_>")
            .nth(1)
            .expect("durable completion has one implementation")
            .split("fn ingress_identity_matches_round")
            .next()
            .expect("response helpers follow durable completion");
        assert!(durable_impl.contains("fn commit_after_exact_dequeue("));
        assert_eq!(
            production.matches("fn commit_after_exact_dequeue(").count(),
            1,
            "only the receipt-bound token may own the post-CAS commit"
        );

        let installed_completion = production
            .split("struct CertifiedFetchCompletion {")
            .nth(1)
            .expect("installed completion has one declaration")
            .split("impl CertifiedFetchCompletion")
            .next()
            .expect("installed completion validation follows its declaration");
        assert!(installed_completion.contains("durable_receipt: DurableBodyReceipt"));
        assert!(installed_completion.contains("replay_evidence: CertifiedFetchReplayEvidenceV1"));
        assert!(installed_completion.contains(".project_durable_ready_fetch("));
        assert!(!installed_completion.contains("CertifiedFetchDequeuedResponse"));

        let durable_binding = production
            .split("fn durable_receipt_matches_fetch(")
            .nth(1)
            .expect("durable response binding has one helper")
            .split("fn exact_selected_response_matches(")
            .next()
            .expect("exact dequeue validation follows durable binding");
        for required in [
            "receipt.request_hash()",
            "receipt.response_hash()",
            "durable_body.context_id()",
            "durable_body.round()",
            "durable_body.subject()",
            "durable_body.manifest_hash()",
            "fetch_effect_matches_manifest",
        ] {
            assert!(
                durable_binding.contains(required),
                "durable Fetch binding omitted {required}"
            );
        }
    }

    #[test]
    fn recovered_decision_apply_scheduler_attestation_stays_closed_and_io_bounded() {
        let registry = reviewed_lifecycle_work_registry_source_for_test();
        let adapter = include_str!("../v2.rs");
        let recovery = include_str!("../v2_lifecycle_work_registry_validate_recovery.rs");
        let boundary = include_str!("../v2_lifecycle_concrete_admission.rs");
        let scheduler = include_str!("../v2_lifecycle_scheduler_inputs.rs");

        let declaration = registry
            .split_once("pub(super) struct ReadyRecoveredDecisionApplyAttestation")
            .expect("recovered Apply attestation has one closed declaration")
            .1
            .split_once("impl ReadyRecoveredDecisionApplyAttestation")
            .expect("recovered Apply attestation implementation follows its declaration")
            .0;
        for forbidden in [
            "AdapterEffect",
            "ValidatedBodyReceipt",
            "PendingRuntimeEffectBinding",
            "ConcreteWorkAddress",
            "LifecycleDigest",
            "pub demand:",
            "pub dispatch_key:",
        ] {
            assert!(
                !declaration.contains(forbidden),
                "recovered Apply attestation exposed {forbidden}"
            );
        }
        assert!(declaration.contains("demand: ReadyRecoveredDecisionApplyDemand"));
        assert!(declaration.contains("dispatch_key: RecoveredDecisionApplyDispatchKeyV1"));
        assert!(registry.contains("pub(super) const fn demand(&self)"));
        assert!(registry.contains("pub(super) const fn dispatch_key(&self)"));
        assert!(registry.contains("impl Drop for ReadyRecoveredDecisionApplyAttestationSeal"));
        assert!(!registry.contains("recovered_decision_apply_carrier_parts"));

        let classifier = recovery
            .split_once("pub(super) fn attest_ready_recovered_decision_apply(")
            .expect("registry owns one recovered Apply classifier")
            .1
            .split_once("/// Project one exact claimed recovered Decision Apply")
            .expect("recovered Apply classifier ends before dispatch projection")
            .0;
        for required in [
            "exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())",
            "record.key.phase() != LifecyclePhase::Apply",
            "record.stage.kind() != LifecycleStageKind::ApplyDecision",
            ".filter(|candidate| candidate.ordinal == ordinal)",
            "metadata.continuation != super::schema::DurableContinuation::None",
            "ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)",
            "apply.matches_current_ready_record(address, digest, coordinator)",
            "ReadyRecoveredDecisionApplyDemand::BoundedIo",
        ] {
            assert!(
                classifier.contains(required),
                "recovered Apply classifier omitted {required}"
            );
        }
        let dispatch = recovery
            .split_once("pub(super) fn prepare_recovered_decision_apply_dispatch(")
            .expect("registry owns one recovered Apply dispatch projection")
            .1
            .split_once("/// Bind one guarded Applied worker result")
            .expect("recovered Apply dispatch ends before terminal rejoin")
            .0;
        for required in [
            "apply.matches_claimed_record(address, digest, coordinator, lease)",
            "apply.dispatch_key.is_some()",
            "RecoveredDecisionApplyDispatchIdentityV1::new(",
            ".project_recovered_apply_task(identity)",
            "PreparedRecoveredDecisionApplyDispatch",
        ] {
            assert!(
                dispatch.contains(required),
                "recovered Apply dispatch projection omitted {required}"
            );
        }
        for forbidden in [
            "into_pair(",
            "validated_receipt(",
            "apply_effect(",
            "PendingRuntimeEffectBinding",
            "EffectWorkId",
        ] {
            assert!(
                !dispatch.contains(forbidden),
                "recovered Apply dispatch reopened legacy authority {forbidden}"
            );
        }
        let candidate_oracle = adapter
            .split_once("pub(in crate::sumeragi) fn exactly_matches_candidate(")
            .expect("recovered Apply carrier has one fixed candidate oracle")
            .1
            .split_once("#[cfg_attr(not(test), allow(dead_code))]")
            .expect("the next adapter boundary follows the carrier oracle")
            .0;
        assert!(candidate_oracle.contains("self.exact_body_binding()"));
        let body_binding = adapter
            .split_once("fn exact_body_binding(&self) -> bool {")
            .expect("recovered Apply carrier retains one exact body-binding oracle")
            .1
            .split_once("/// Recheck the immutable WAL/body lineage and final effect binding.")
            .expect("carrier validation follows the body-binding oracle")
            .0;
        for required in [
            "exactly_matches_validated_receipt",
            "exactly_binds_adapter_effect",
            "certificate.execution_commitment",
            "self.validated_receipt.execution_commitment()",
        ] {
            assert!(
                body_binding.contains(required),
                "recovered Apply carrier omitted transitive body binding {required}"
            );
        }
        for forbidden in [
            "into_pair(",
            "take_for_lease(",
            "validated_receipt(",
            "apply_effect(",
        ] {
            assert!(
                !classifier.contains(forbidden),
                "recovered Apply classifier opened raw surface {forbidden}"
            );
        }

        let holder = boundary
            .split_once("pub(super) fn attest_ready_recovered_decision_apply(")
            .expect("holder delegates the closed Apply attestation")
            .1
            .split_once("/// Reconstruct one storage-authenticated recovered Validate parent")
            .expect("holder delegation remains narrow")
            .0;
        assert!(holder.contains(".attest_ready_recovered_decision_apply(coordinator, ordinal)"));
        assert!(!holder.contains("registry_mut"));

        let apply_branch = scheduler
            .split_once("LifecycleWorkClass::Apply =>")
            .expect("direct scheduler classifies recovered Apply")
            .1
            .split_once("_ =>")
            .expect("unsupported direct carriers follow the Apply branch")
            .0;
        let attest = apply_branch
            .find("attest_ready_recovered_decision_apply")
            .expect("Apply branch consumes the exact registry attestation");
        let demand = apply_branch
            .find("ReadyRecoveredDecisionApplyDemand::BoundedIo")
            .expect("Apply branch matches the typed I/O demand");
        let reject = apply_branch
            .find("ProductionSchedulerInputsError::IoCapacityObservationRequired")
            .expect("Apply cannot be claimed before bounded I/O capacity is joined");
        assert!(attest < demand && demand < reject);
        assert!(!apply_branch.contains("Some(attestation)"));
    }

    #[test]
    fn recovered_decision_apply_terminal_settlement_is_exact_and_post_fsync_infallible() {
        let registry = include_str!("../v2_lifecycle_work_registry_validate_recovery.rs");
        let adapter = include_str!("../v2.rs");
        let runtime = include_str!("../v2_runtime.rs");
        let executor = include_str!("../v2_effects.rs");
        let launch = include_str!("../v2_lifecycle_launch.rs");
        let lane = include_str!("../v2_lane_work.rs");

        let completion_projection = adapter
            .split_once("pub(in crate::sumeragi) fn project_recovered_apply_completion(")
            .expect("the closed Apply carrier has one completion projection")
            .1
            .split_once("impl PreparedRecoveredDecisionApplyAdapterCompletionV1")
            .expect("the fixed adapter commit follows completion projection")
            .0;
        for required in [
            "self.exact_body_binding()",
            "key.matches_carrier(self.context, self.installed_digest())",
            "completion.subject()",
            "completion.certificate()",
            "completion.validated_receipt()",
            "artifact.validate()",
            "receipt.artifact_hash() != HashOf::new(artifact)",
        ] {
            assert!(
                completion_projection.contains(required),
                "completion projection omitted {required}"
            );
        }

        let prepare = registry
            .split_once("pub(super) fn prepare_recovered_decision_apply_terminal_transition(")
            .expect("the registry has one exact completion rejoin")
            .1
            .split_once("/// Publish one exact recovered Apply terminal")
            .expect("terminal publication follows completion rejoin")
            .0;
        for required in [
            "coordinator.active_lease.as_ref() != Some(lease)",
            "apply.matches_claimed_record(address, digest, coordinator, lease)",
            "apply.dispatch_key != Some(dispatch_key)",
            "dispatch_key.matches(coordinator.active_context, address, digest)",
            "RecoveredDecisionApplyCompletionProjectionPermit::new()",
            ".project_recovered_apply_completion(",
        ] {
            assert!(prepare.contains(required), "completion rejoin omitted {required}");
        }

        let publication = registry
            .split_once("pub(super) fn publish_recovered_decision_apply_terminal_transition")
            .expect("the registry has one terminal publication cut")
            .1
            .split_once("/// Prepare execution of one exact Ready durable Validate completion.")
            .expect("terminal publication ends before Validate execution")
            .0;
        for required in [
            "let exact_current =",
            "let mut expected = current.stage_durable_transaction()",
            "expected.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None)",
            "expected.records == staged.records",
            "expected.key_index == staged.key_index",
            "expected.owner_index == staged.owner_index",
            "expected.ready_index == staged.ready_index",
            "expected.durable_records == staged.durable_records",
            "expected.capacity_used == staged.capacity_used",
            "expected.producer_debts == staged.producer_debts",
            "match publish()",
            ".remove(&prepared.address)",
        ] {
            assert!(publication.contains(required), "publication omitted {required}");
        }
        let preflight = publication.find("if !exact_current || !exact_staged").unwrap();
        let fsync = publication.find("match publish()").unwrap();
        let removal = publication.find(".remove(&prepared.address)").unwrap();
        assert!(preflight < fsync && fsync < removal);

        let adapter_preview = adapter
            .split_once("pub(in crate::sumeragi) fn prepare_recovered_decision_apply_completion(")
            .expect("the adapter has one recovered Apply preview")
            .1
            .split_once("/// Decide whether an exact internal callback")
            .expect("the recovered Apply preview stays bounded")
            .0;
        for required in [
            "let mut next_registry = self.registry.clone()",
            "let mut next_reducer = self.reducer.clone()",
            "reducer::Event::ApplicationCompleted",
            "outcome.disposition() != reducer::StepDisposition::Applied",
            "!outcome.effects().is_empty()",
            "self.pending_persistence_id.is_some()",
            "!self.deferred_completions.is_empty()",
            "!self.deferred_progress_inputs.is_empty()",
            "!self.deferred_inputs.is_empty()",
            "!next_reducer.ready_to_finish()",
            "core::mem::swap(&mut self.reducer, &mut next_reducer)",
            "let committed_status = self.status()",
            "self.last_progress = prior_last_progress",
        ] {
            assert!(adapter_preview.contains(required), "adapter preview omitted {required}");
        }
        assert!(!adapter_preview.contains("self.step("));
        assert!(runtime.contains("fn prepare_recovered_decision_apply_completion("));
        assert!(executor.contains("fn commit_recovered_decision_apply_finality("));

        let settlement = launch
            .split_once("pub(in crate::sumeragi) fn settle_recovered_decision_apply_completion(")
            .expect("the launched owner has one terminal settlement")
            .1
            .split_once("/// Fail-stop failure while consuming the recovered lifecycle owner")
            .expect("terminal settlement ends before launch errors")
            .0;
        for required in [
            "drain_recovered_decision_apply_completion()",
            "RecoveredDecisionApplyWorkerResultV1::Deferred { task, reference }",
            "completion.authorizes_sidecar_owner(services, lane_work)",
            "sidecar.register(lane_work)",
            "MergeSidecarDeferralDisposition::Fetching",
            "MergeSidecarDeferralDisposition::RetryLater",
            "RecoveredDecisionApplyWorkerResultV1::Applied(applied)",
            "prepare_recovered_decision_apply_terminal_transition(",
            "executor.prepare_recovered_decision_apply_completion(authority)",
            "staged.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None)",
            "publish_recovered_decision_apply_terminal_transition(",
            "persist_exact_staged_successor(&staged)",
            "owner.coordinator = staged",
            "adapter.commit_after_durable_settlement()",
            "executor.commit_recovered_decision_apply_finality(finality)",
            "completion.acknowledge_after_owner_settlement()",
            "status::set_v2_status(status)",
        ] {
            assert!(settlement.contains(required), "settlement omitted {required}");
        }
        let deferred_retry = launch
            .split_once("impl RetainedRecoveredDecisionApplyDeferredV1")
            .expect("the deferred Apply owner has one retry implementation")
            .1
            .split_once("/// Fail-stop class while durably terminalizing")
            .expect("the deferred retry stays bounded")
            .0;
        let requeue = deferred_retry.find("completion.retry_deferred()").unwrap();
        assert!(deferred_retry.contains("fn retry_after_available(self)"));
        assert!(!deferred_retry.contains("dispatch_lane_work_effects("));
        assert!(!deferred_retry.contains("effect_limit"));
        let drive = launch
            .split_once("fn drive_recovered_decision_apply_deferred(")
            .expect("the launched owner has one sealed sidecar drive")
            .1
            .split_once("/// Exercise recovered Apply dispatch")
            .expect("the sidecar drive stays bounded")
            .0;
        for required in [
            "authorizes_sidecar_owner(&self.services, lane_work)",
            "deferred.sidecar.register(lane_work)",
            "dispatch_next_recovered_apply_sidecar_request(",
            "deferred.retry_after_available()",
        ] {
            assert!(drive.contains(required), "sidecar drive omitted {required}");
        }
        assert!(!drive.contains("dispatch_lane_work_effects("));
        assert!(!drive.contains("effect_limit"));
        let owner = drive.find("authorizes_sidecar_owner").unwrap();
        let sidecar = drive.find("deferred.sidecar.register(lane_work)").unwrap();
        let available = drive
            .find("MergeSidecarDeferralDisposition::Available")
            .unwrap();
        assert!(owner < sidecar && sidecar < available);
        assert!(requeue < deferred_retry.len());
        let exact_request = lane
            .split_once("fn dispatch_next_recovered_apply_sidecar_request(")
            .expect("lane owner has one exact recovered Apply request dispatcher")
            .1
            .split_once("#[cfg(test)]")
            .expect("exact request dispatch ends before test-only authority")
            .0;
        for required in [
            "let Some(next) = self.next_effect()",
            "CertifiedMergeSidecarMessage::Request(request)",
            "request.request_id != request.canonical_request_id()",
            "request.entry_hash != reference.entry_hash",
            "request.reference_digest != certified_merge_reference_digest(reference)",
            ".can_retain_lane_work_effect(&next)",
            "let Some(drained) = self.drain_effects(1).pop()",
            "ExactFanoutOwnership::SourceRetained",
            "self.requeue_effect(",
        ] {
            assert!(
                exact_request.contains(required),
                "exact sidecar request dispatch omitted {required}"
            );
        }
        let peek = exact_request.find("let Some(next) = self.next_effect()").unwrap();
        let preflight = exact_request.find(".can_retain_lane_work_effect(&next)").unwrap();
        let drain = exact_request
            .find("let Some(drained) = self.drain_effects(1).pop()")
            .unwrap();
        let post = exact_request
            .find(".post_certified_merge_sidecar_with_reply_routes(")
            .unwrap();
        assert!(peek < preflight && preflight < drain && drain < post);
        assert!(!exact_request.contains("dispatch_lane_work_effects("));
        assert!(!exact_request.contains("for _ in"));
        let publish = settlement
            .find("publish_recovered_decision_apply_terminal_transition(")
            .unwrap();
        let coordinator = settlement.find("owner.coordinator = staged").unwrap();
        let adapter = settlement
            .find("adapter.commit_after_durable_settlement()")
            .unwrap();
        let executor = settlement
            .find("executor.commit_recovered_decision_apply_finality(finality)")
            .unwrap();
        let acknowledgement = settlement
            .find("completion.acknowledge_after_owner_settlement()")
            .unwrap();
        let status = settlement.rfind("status::set_v2_status(status)").unwrap();
        assert!(
            publish < coordinator
                && coordinator < adapter
                && adapter < executor
                && executor < acknowledgement
                && acknowledgement < status
        );
        for forbidden in [
            "complete_application(",
            "EffectWorkId",
            "RuntimeEffectOwnership",
            "drain_completions(",
        ] {
            assert!(
                !settlement.contains(forbidden),
                "terminal settlement reopened {forbidden}"
            );
        }
    }

    #[test]
    fn recovered_next_wal_vote_completion_stays_closed_and_attests_its_ready_pair() {
        let replay = include_str!("../v2_lifecycle_replay_authority.rs");
        let closed = replay
            .split_once(
                "struct RecoveredLifecycleNextWalVoteSignedBroadcastProjectionV1",
            )
            .expect("next-WAL Vote signed successor is closed")
            .1
            .split_once("/// Canonical structural evidence for a recovered ProposalIntent")
            .expect("closed signed successor has a bounded surface")
            .0;
        for required in [
            "effect: AdapterEffect",
            "pending: PendingRuntimeEffectBinding",
            "candidate: CandidateAdmission",
            "RecoveredLifecycleSignBroadcastProjectionPermitV1",
        ] {
            assert!(closed.contains(required), "closed projection omitted {required}");
        }
        let oracle = replay
            .split_once("fn project_authenticated_signed_broadcast(")
            .expect("next-WAL Vote has one signed successor oracle")
            .1
            .split_once("/// Recheck a closed signed child")
            .expect("signed successor oracle stays bounded")
            .0;
        for required in [
            "self.is_exact(verified)",
            "verified.verify_consensus_message(message)",
            ".project_signed_broadcast_successor(&self.seal.effect, &broadcast)",
            "exact_signed_broadcast_successor_candidate(verified, &broadcast, &pending)",
        ] {
            assert!(oracle.contains(required), "signed oracle omitted {required}");
        }

        let wal = include_str!("../v2_lifecycle_wal_recovery.rs");
        for seam in [
            "fn project_recovered_next_wal_vote_signed_broadcast(",
            "fn project_recovered_next_wal_vote_signed_broadcast_and_sign(",
        ] {
            let body = wal
                .split_once(seam)
                .unwrap_or_else(|| panic!("WAL recovery omitted {seam}"))
                .1
                .split_once("\n}")
                .expect("WAL seam has one bounded body")
                .0;
            assert!(body.contains("parent.project_authenticated_signed_broadcast"));
            assert!(body.contains("RecoveredLifecycleSignBroadcastProjectionPermitV1::new()"));
            assert!(body.contains("cold_proposal_output: None"));
        }

        let registry = include_str!("../v2_lifecycle_work_registry.rs");
        let single = registry
            .split_once("fn prepare_recovered_lifecycle_sign_broadcast_successor")
            .expect("single successor preparation exists")
            .1
            .split_once("/// Seal the exact Broadcast-and-next-WAL-Sign pair")
            .expect("single successor preparation stays bounded")
            .0;
        let combined = registry
            .split_once("fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor")
            .expect("combined successor preparation exists")
            .1
            .split_once(
                "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor",
            )
            .expect("combined successor preparation stays bounded")
            .0;
        assert!(single.contains("DurableRecoveredLifecycleNextWalVoteSign(sign)"));
        assert!(single.contains("project_recovered_next_wal_vote_signed_broadcast("));
        assert!(combined.contains("DurableRecoveredLifecycleNextWalVoteSign(sign)"));
        assert!(combined.contains(
            "project_recovered_next_wal_vote_signed_broadcast_and_sign("
        ));
        assert!(registry.contains(
            "DurableRecoveredLifecycleSignParentV1::NextWalVote(sign)"
        ));

        let recovery = include_str!("../v2_lifecycle_work_registry_validate_recovery.rs");
        let pair = recovery
            .split_once(
                "fn attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(",
            )
            .expect("cold Ready pair attestation exists")
            .1
            .split_once("/// Project the exact claimed Broadcast")
            .expect("cold Ready pair attestation stays bounded")
            .0;
        for required in [
            "broadcast_ordinal.checked_add(1) != Some(next_sign_ordinal)",
            "broadcast_record.owner == next_sign_record.owner",
            "attest_ready_recovered_lifecycle_signed_broadcast(coordinator, broadcast_ordinal)",
            "attest_ready_recovered_lifecycle_sign(coordinator, next_sign_ordinal)",
            "DurableRecoveredLifecycleNextWalVoteSign(next_sign)",
            "next_sign.matches_current_ready_record(",
        ] {
            assert!(pair.contains(required), "pair attestation omitted {required}");
        }
    }
