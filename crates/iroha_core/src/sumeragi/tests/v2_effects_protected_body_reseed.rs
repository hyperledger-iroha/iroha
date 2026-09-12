// Protected durable-body recovery across Prepare and Commit authority.

#[test]
fn protected_commit_validate_reseeds_missing_replay_without_applying() {
    let fixture = Fixture::new();
    for active_view in [0, 2] {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (key, durable) =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let effect = AdapterEffect::ValidateBody {
            tag: tag(active_view),
            round: key.0,
            subject: key.1,
        };
        let ownership = recovered_validate_retry_ownership(
            &fixture,
            &effect,
            Some(commit.clone()),
            9_130 + u128::from(active_view),
        );
        let decision = (
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        );
        executor.protected_decision = Some(decision);
        executor.runtime.decided_body = Some(decision);
        // This fixture starts after the active view has been installed.
        executor.runtime.round_tag = Some(tag(active_view));
        executor.reconciled_tag = Some(tag(active_view));
        executor.runtime.durable_body_authority_certificate = Some(commit);
        executor.runtime.exact_effect_ownership = Some((effect.clone(), ownership.clone()));
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("the protected CommitQC admits validation of the exact durable body");
        let pending = &executor.pending_durable_validate_admissions[&key];
        assert!(pending.exactly_matches_retry(&effect, &ownership));
        assert!(!pending.projects_local_proposal_handoff_for_test());
        assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
        assert!(executor.validated_bodies.is_empty());
        assert!(matches!(
            &executor.durable_validate_retry_seals[&key],
            DurableValidateRetrySealV1::Live {
                store_terminal: Some(_),
                lifecycle_state: DurableValidateRetryLifecycleStateV1::PendingAdmission,
                ..
            }
        ));
        assert!(services.fetch_tasks.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(services.apply_tasks.is_empty());
        assert!(executor.pending_applications.is_empty());
        assert!(!executor.output_guard.restart_required());
    }
}

#[test]
fn missing_replay_commit_rejects_foreign_decision_and_commitment() {
    let fixture = Fixture::new();
    for (wrong_decision, wrong_commitment) in [(false, false), (true, false), (false, true)] {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let _recovered =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        let effect = AdapterEffect::ValidateBody {
            tag: tag(0),
            round: commit.proposal_round,
            subject: commit.subject,
        };
        let ownership =
            recovered_validate_retry_ownership(&fixture, &effect, Some(commit.clone()), 9_139);
        let mut retained = commit.clone();
        let expected = if wrong_commitment {
            retained.execution_commitment =
                wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                    Hash::new(b"foreign Commit parent"),
                    Hash::new(b"foreign Commit post"),
                    Hash::new(b"foreign Commit writes"),
                    1,
                    Hash::new(b"foreign Commit block"),
                );
            "changed its durable QC coordinates"
        } else {
            "is not the protected durable body"
        };
        if wrong_decision || wrong_commitment {
            let decision = (
                retained.round,
                retained.proposal_round,
                if wrong_decision {
                    distinct_body(&fixture).0
                } else {
                    retained.subject
                },
                retained.execution_commitment,
            );
            executor.protected_decision = Some(decision);
            executor.runtime.decided_body = Some(decision);
        }
        executor.runtime.durable_body_authority_certificate = Some(retained);
        if wrong_decision {
            // The authority join rejects the foreign Decision. Normal dispatch
            // then retires its stale body effect before it can reach validation.
            let before = executor.body_ownership_projection();
            let result =
                executor.exact_remote_proposal_validate_authority_certificate(&effect, &ownership);
            assert!(
                matches!(
                    &result,
                    Err(EffectExecutorError::Contract(reason)) if reason.contains(expected)
                ),
                "expected {expected:?}, got {result:?}"
            );
            executor.runtime.exact_effect_ownership = Some((effect.clone(), ownership));
            assert_eq!(
                executor
                    .consume_effects(vec![effect], &mut services)
                    .expect("a foreign decided body retires the stale Validate effect"),
                0,
            );
            assert_eq!(executor.body_ownership_projection(), before);
            assert!(executor.pending_durable_validate_admissions.is_empty());
            assert!(executor.durable_validate_retry_seals.is_empty());
            assert!(services.fetch_tasks.is_empty());
            assert!(services.store_tasks.is_empty());
            assert!(services.apply_tasks.is_empty());
            assert!(!executor.output_guard.restart_required());
            continue;
        }
        assert_missing_replay_validate_fails_closed_without_body_mutation(
            &mut executor,
            &mut services,
            effect,
            ownership,
            expected,
        );
    }
}
