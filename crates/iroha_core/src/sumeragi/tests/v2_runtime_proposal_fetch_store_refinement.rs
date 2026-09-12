#[test]
#[allow(clippy::too_many_lines)]
fn authenticated_proposal_store_retains_root_after_fetch_or_queued_completion_upgrade() {
    for upgrade_queued_completion in [false, true] {
        for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
            let directory = TempDir::new().expect("temporary Proposal Store refinement directory");
            let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
                &directory,
                RuntimeQueueConfig::new(8, 1, 1),
                Some(0),
            );
            let now = Instant::now();
            runtime.arm_live_clocks(now).expect("arm the real runtime");
            runtime
                .enqueue_network(signed_runtime_proposal(&context, &keys, 0xB7))
                .expect("admit the signed Proposal through authenticated ingress");
            let RuntimeStep::Advanced(effects) = runtime.step(now).expect("dispatch Proposal")
            else {
                panic!("authenticated Proposal unexpectedly idled")
            };
            runtime.take_last_scheduler_ownership().unwrap();
            let [fetch] = effects.as_slice() else {
                panic!("Proposal must emit one Fetch: {effects:?}")
            };
            let AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                certificate: None,
                ..
            } = fetch
            else {
                panic!("Proposal must emit one ordinary manifest-bound Fetch")
            };
            let tag = *tag;
            let manifest = manifest.clone();
            let original = runtime
                .take_effect_ownership(effects.len())
                .unwrap()
                .remove(0);
            let replay = original
                .exact_remote_proposal_fetch_replay(fetch)
                .expect("the real receiver attaches its exact signed Proposal seal");
            let mut certificate = signed_runtime_quorum_certificate(&context, &keys, 0xB7);
            certificate.phase = phase;
            certificate.round = manifest.round;
            certificate.proposal_round = manifest.round;
            certificate.subject = manifest.subject;
            let preimage = wire::Vote {
                round: certificate.round,
                proposal_round: certificate.proposal_round,
                phase,
                subject: certificate.subject,
                execution_commitment: certificate.execution_commitment,
                signer: certificate.signers[0],
                signature: Vec::new(),
            }
            .signature_preimage();
            let shares = certificate
                .signers
                .iter()
                .map(|signer| {
                    Signature::new(
                        keys[usize::try_from(*signer).unwrap()].private_key(),
                        &preimage,
                    )
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
            certificate.aggregate_signature =
                iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                    .expect("aggregate the exact certified Fetch authority");
            certificate
                .validate(&context)
                .expect("valid frozen-roster QC");
            let certified = AdapterEffect::FetchBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect(),
                certificate: Some(certificate),
            };
            let upgraded = original
                .rebind_same_adapter_effect(&certified)
                .expect("retain the physical Fetch root while refining its candidate authority");
            let first_owner = if upgrade_queued_completion {
                &original
            } else {
                &upgraded
            };
            let reservation = runtime
                .reserve_body_available_with_owner(tag, manifest.clone(), first_owner)
                .expect("reserve the exact reconstructed body under the physical Fetch owner");
            runtime
                .commit_body_available(reservation)
                .expect("publish the unique BodyAvailable completion");
            if upgrade_queued_completion {
                let retry = runtime
                    .reserve_body_available_with_owner(tag, manifest.clone(), &upgraded)
                    .expect("a queued completion accepts its exact monotonic authority upgrade");
                assert!(!retry.owns_new_slot());
                runtime
                    .commit_body_available(retry)
                    .expect("the upgraded completion retains its one FIFO slot");
            }
            let RuntimeStep::Advanced(store_effects) = runtime
                .step(now)
                .expect("dispatch the retained BodyAvailable")
            else {
                panic!("BodyAvailable unexpectedly idled")
            };
            runtime.take_last_scheduler_ownership().unwrap();
            let [store] = store_effects.as_slice() else {
                panic!("BodyAvailable must emit one Store: {store_effects:?}")
            };
            assert!(matches!(store, AdapterEffect::StoreBody { .. }));
            let successor = runtime
                .take_effect_ownership(store_effects.len())
                .unwrap()
                .remove(0);
            assert_eq!(successor.owner(), original.owner());
            assert_eq!(
                successor.candidate_semantic_statement(),
                upgraded.candidate_semantic_statement(),
            );
            let pending = successor
                .exact_pending_adapter_effect_binding(store)
                .unwrap();
            assert!(replay.exactly_projects_store(store, &pending));
            let stored = replay
                .project_exact_store(store, &pending)
                .expect("the original Proposal seal follows the upgraded physical successor");
            assert!(stored.exactly_matches_store_pending(store, &pending));
            assert!(!runtime.fail_closed);
        }
    }
}
