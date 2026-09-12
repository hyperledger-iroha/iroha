#[test]
#[allow(clippy::too_many_lines)]
fn hybrid_proposal_fetch_completes_store_and_validate_with_exact_replay_root() {
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor.runtime.retain_body_available_effect_ownership = true;
        let key = (fixture.manifest.round, fixture.manifest.subject);
        let (fetch, original, _) = prepared_remote_proposal_fetch_replay(&fixture, tag(0), 98_101);
        let original_pending = original
            .exact_pending_adapter_effect_binding(&fetch)
            .unwrap();
        executor
            .retain_effect_batch(vec![fetch], vec![original.clone()])
            .expect("retain the authenticated ordinary Proposal Fetch");
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("start the ordinary acquisition");
        let work_id = services.fetch_tasks[0].id();
        let certificate = fixture.qc(phase);
        let certified = AdapterEffect::FetchBody {
            tag: tag(0),
            round: key.0,
            subject: key.1,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: certified_sources(&fixture, &certificate),
            certificate: Some(certificate.clone()),
        };
        let incoming = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&certified),
            vec![
                RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                    tag(0),
                    98_102,
                    b"independent authenticated QC joins the live Proposal acquisition",
                ),
            ],
        )
        .expect("bind a separately admitted certified Fetch")
        .pop()
        .expect("one certified Fetch owner");
        assert_ne!(incoming.owner(), original.owner());
        assert_ne!(
            incoming
                .exact_pending_adapter_effect_binding(&certified)
                .unwrap()
                .causal_lifecycle_key(),
            original_pending.causal_lifecycle_key(),
        );
        executor
            .retain_effect_batch(vec![certified], vec![incoming])
            .expect("join stronger authority to the incumbent physical Fetch");
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("publish the hybrid acquisition without replacing its owner");
        let hybrid = executor.pending_fetches[&work_id].task.clone();
        assert_eq!(hybrid.ownership().owner(), original.owner());
        let hybrid_pending = hybrid
            .ownership()
            .exact_pending_adapter_effect_binding(&hybrid.adapter_effect())
            .unwrap();
        assert_ne!(
            hybrid_pending.candidate_statement(),
            original_pending.candidate_statement(),
        );
        assert_eq!(
            executor
                .complete_body_reconstruction(
                    &hybrid,
                    fixture.manifest.clone(),
                    fixture.body.clone(),
                    &mut services,
                )
                .expect("authenticated Proposal reconstruction completes the hybrid Fetch"),
            CompletionDisposition::Accepted,
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::BodyAvailable(_)),
        ));
        let store = AdapterEffect::StoreBody {
            tag: tag(0),
            round: key.0,
            subject: key.1,
        };
        executor.runtime.completions.clear();
        executor
            .consume_effects(vec![store.clone()], &mut services)
            .expect("hybrid BodyAvailable projects its exact Store successor");
        assert_eq!(services.store_tasks.len(), 1);
        let stored_task = services.store_tasks[0].clone();
        assert_eq!(stored_task.ownership().owner(), original.owner());
        assert_eq!(
            stored_task
                .ownership()
                .exact_pending_adapter_effect_binding(&store)
                .unwrap()
                .candidate_statement(),
            hybrid_pending.candidate_statement(),
        );
        let completion = services.execute_store(stored_task.id());
        executor
            .complete_body_store(completion, &mut services)
            .expect("Store fsync advances the same Proposal replay family");
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::Stored { .. }),
        ));
        let validate = AdapterEffect::ValidateBody {
            tag: tag(0),
            round: key.0,
            subject: key.1,
        };
        let validate_owner = stored_task
            .ownership()
            .rebind_as_inherited_adapter_effect(&validate)
            .expect("Store retains its certified candidate at Validate");
        executor.runtime.completions.clear();
        // The full QC must also be the reducer's exact durable authority.
        // Observed current Prepare authorizes validation before a voting
        // lock exists; Commit requires the protected durable Decision.
        match phase {
            wire::GlobalPhase::Prepare => {
                executor.runtime.highest_prepare = Some(certificate.as_ref());
                executor.runtime.current_prepare_authority_certificate = Some(certificate.clone());
            }
            wire::GlobalPhase::Commit => {
                let decision = (
                    certificate.round,
                    certificate.proposal_round,
                    certificate.subject,
                    certificate.execution_commitment,
                );
                executor.protected_decision = Some(decision);
                executor.runtime.decided_body = Some(decision);
            }
        }
        executor.runtime.durable_body_authority_certificate = Some(certificate);
        executor
            .retain_effect_batch(vec![validate], vec![validate_owner])
            .expect("retain the exact durable Validate successor");
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("join Proposal body durability with the registered QC authority");
        assert!(executor.remote_proposal_replay.is_empty());
        assert!(
            executor
                .pending_durable_validate_admissions
                .contains_key(&key)
        );
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn proposal_fetch_store_refinement_rejects_foreign_root_and_coordinates() {
    let fixture = Fixture::new();
    let (fetch, original, _) = prepared_remote_proposal_fetch_replay(&fixture, tag(0), 98_103);
    let replay = original
        .exact_remote_proposal_fetch_replay(&fetch)
        .expect("retain the receiver-authenticated Proposal origin");
    let store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
        let certificate = fixture.qc(phase);
        let certified = AdapterEffect::FetchBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            certified_sources: certified_sources(&fixture, &certificate),
            certificate: Some(certificate),
        };
        let upgraded = original
            .rebind_same_adapter_effect(&certified)
            .expect("bind the exact certified Fetch under its incumbent root");
        let successor = upgraded
            .rebind_as_inherited_adapter_effect(&store)
            .expect("project its exact Store carrier");
        let pending = successor
            .exact_pending_adapter_effect_binding(&store)
            .expect("sealed exact Store binding");
        assert!(replay.exactly_projects_store(&store, &pending));
        let stored = replay
            .clone()
            .project_exact_store(&store, &pending)
            .expect("project the upgraded Store with its original Proposal seal");
        let receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.manifest.round,
            fixture.manifest.subject,
            HashOf::new(&fixture.manifest),
        );
        let stored = stored
            .bind_durable_body(&store, &receipt)
            .expect("bind the exact durable Proposal body");
        let validate = AdapterEffect::ValidateBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        };
        let validate_pending = successor
            .rebind_as_inherited_adapter_effect(&validate)
            .unwrap()
            .exact_pending_adapter_effect_binding(&validate)
            .unwrap();
        assert!(
            stored
                .project_exact_validate(&store, &receipt, &validate, &validate_pending, None)
                .is_err(),
            "a refined Store does not replace the registered QC needed by Validate",
        );
        let foreign = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&certified),
            vec![
                RuntimeEffectOwnership::fresh_for_test_with_semantic_identity(
                    tag(0),
                    98_104,
                    b"independent certified Fetch for foreign Proposal-root rejection",
                ),
            ],
        )
        .expect("bind an independently rooted certified Fetch")
        .pop()
        .expect("one foreign Fetch owner")
        .rebind_as_inherited_adapter_effect(&store)
        .expect("bind a foreign Store root");
        let foreign_pending = foreign
            .exact_pending_adapter_effect_binding(&store)
            .unwrap();
        assert_ne!(
            pending.causal_lifecycle_key(),
            foreign_pending.causal_lifecycle_key(),
            "the negative fixture must differ in causal root, not only lifecycle ordinal",
        );
        assert!(!replay.exactly_projects_store(&store, &foreign_pending));
        for altered in [
            AdapterEffect::StoreBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
            AdapterEffect::StoreBody {
                tag: tag(0),
                round: round(&fixture.context, 1),
                subject: fixture.manifest.subject,
            },
        ] {
            assert!(!replay.exactly_projects_store(&altered, &pending));
        }
        let later_tag_store = AdapterEffect::StoreBody {
            tag: EventTag::new(
                tag(0).height(),
                tag(0).view(),
                Generation::new(tag(0).generation().get() + 1),
            ),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        };
        let later_tag_pending = upgraded
            .rebind_as_inherited_adapter_effect(&later_tag_store)
            .expect("bind a changed generation under the same causal root")
            .exact_pending_adapter_effect_binding(&later_tag_store)
            .unwrap();
        assert!(!replay.exactly_projects_store(&later_tag_store, &later_tag_pending));
        let certified_pending = upgraded
            .exact_pending_adapter_effect_binding(&certified)
            .expect("certified Fetch binding");
        assert!(
            certified_pending
                .project_proposal_fetch_store_successor_with_authority_refinement(
                    &certified, &store, &pending,
                )
                .is_none(),
            "a certified Fetch cannot mint an ordinary Proposal origin",
        );
    }
}
