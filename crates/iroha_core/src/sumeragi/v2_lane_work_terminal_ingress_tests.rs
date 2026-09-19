// Terminal authentication and owner retirement on canonical first-release sources.

fn run_terminal_ingress_test(name: &str, test: fn()) {
    let handle = crate::sumeragi::sumeragi_thread_builder(name)
        .spawn(test)
        .expect("spawn terminal ingress test on the production consensus stack");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn terminal_ready_authentication_rejects_wrong_role_payload_and_cryptography() {
    run_terminal_ingress_test("terminal-ready-authentication", || {
        // These are the exact pure validators used before terminal Duplicate.
        // Their cryptographic contract does not require manufacturing an applied receipt.
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (source, mut proposal) =
            planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
        proposal.payload_block_hint = None;
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let input = source.external_entrypoints_cloned().next().unwrap();
        let (payload, _) = signed_autonomous_payload_for_entrypoint(
            &adapter,
            &keys,
            &proposal,
            input,
            b"terminal-authentication-admission",
            b"terminal-authentication-owner",
            "canonical producer",
            "producer key",
            "signed canonical source",
        );
        assert_eq!(keys.len(), 4);
        let prepare = keys[..3]
            .iter()
            .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
            .collect::<Vec<_>>();
        let commit = keys[..3]
            .iter()
            .map(|key| signed_lane_vote(&proposal, CertPhase::Commit, key))
            .collect::<Vec<_>>();
        let aggregate = |phase, votes: &[LaneBlockVoteV1]| {
            crate::lane_consensus::aggregate_lane_block_votes_to_qc(
                proposal.vote_body(phase),
                proposal.descriptor.validator_set.clone(),
                votes,
            )
            .expect("exact three-of-four authenticated quorum")
        };
        let prepare_qc = aggregate(CertPhase::Prepare, &prepare);
        let commit_qc = aggregate(CertPhase::Commit, &commit);
        let pops = keys
            .iter()
            .map(|key| {
                (
                    key.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
                )
            })
            .collect();
        for vote in prepare.iter().chain(&commit) {
            validate_terminal_autonomous_vote(vote, &payload).unwrap();
            let mut corrupt = vote.clone();
            corrupt.bls_signature[0] ^= 0x80;
            assert!(validate_terminal_autonomous_vote(&corrupt, &payload).is_err());
        }
        for qc in [&prepare_qc, &commit_qc] {
            validate_terminal_autonomous_qc(qc, &payload, &pops).unwrap();
            let mut corrupt = qc.clone();
            corrupt.bls_aggregate_signature[0] ^= 0x80;
            assert!(validate_terminal_autonomous_qc(&corrupt, &payload, &pops).is_err());
        }
        for mutation in 0..3 {
            let mut vote = prepare[0].clone();
            match mutation {
                0 => vote.payload_availability_vote = None,
                1 => {
                    vote.payload_availability_vote
                        .as_mut()
                        .unwrap()
                        .bls_signature[0] ^= 0x80
                }
                2 => {
                    vote.payload_availability_vote
                        .as_mut()
                        .unwrap()
                        .validator_set_pops[0][0] ^= 0x80
                }
                _ => unreachable!(),
            }
            assert!(
                validate_terminal_autonomous_vote(&vote, &payload).is_err(),
                "vote mutation {mutation}"
            );
            let mut qc = prepare_qc.clone();
            match mutation {
                0 => qc.payload_availability_qc = None,
                1 => {
                    qc.payload_availability_qc
                        .as_mut()
                        .unwrap()
                        .bls_aggregate_signature[0] ^= 0x80
                }
                2 => {
                    qc.payload_availability_qc
                        .as_mut()
                        .unwrap()
                        .validator_set_pops[0][0] ^= 0x80
                }
                _ => unreachable!(),
            }
            assert!(
                validate_terminal_autonomous_qc(&qc, &payload, &pops).is_err(),
                "QC mutation {mutation}"
            );
        }
        let mut wrong_commit = commit[0].clone();
        wrong_commit.payload_availability_vote = prepare[0].payload_availability_vote.clone();
        assert!(validate_terminal_autonomous_vote(&wrong_commit, &payload).is_err());
        let mut wrong_commit_qc = commit_qc.clone();
        wrong_commit_qc.payload_availability_qc = prepare_qc.payload_availability_qc.clone();
        assert!(validate_terminal_autonomous_qc(&wrong_commit_qc, &payload, &pops).is_err());
        let mut wrong_body = prepare[0]
            .payload_availability_vote
            .as_ref()
            .unwrap()
            .body
            .clone();
        wrong_body.executable_payload_hash =
            Hash::new(b"independently authenticated different payload");
        let wrong_votes = keys[..3]
            .iter()
            .map(|key| {
                signed_autonomous_prepare_vote_for_body(&proposal, wrong_body.clone(), key, &keys)
            })
            .collect::<Vec<_>>();
        for vote in &wrong_votes {
            vote.validate_ingress(CertPhase::Prepare).unwrap();
            assert!(validate_terminal_autonomous_vote(vote, &payload).is_err());
        }
        let wrong_qc = aggregate(CertPhase::Prepare, &wrong_votes);
        validate_winning_lane_qc(&wrong_qc, &proposal, &pops).unwrap();
        assert!(validate_terminal_autonomous_qc(&wrong_qc, &payload, &pops).is_err());
    });
}

#[test]
fn canonical_ordinary_terminal_ingress_preserves_member_observer_and_full_output_owners() {
    run_terminal_ingress_test(
        "canonical-terminal-ingress",
        canonical_ordinary_terminal_ingress,
    );
}

fn canonical_ordinary_terminal_ingress() {
    let (state, kura, keys, block, context, successor) =
        crate::sumeragi::v2_apply::canonical_ordinary_terminal_fixture_for_test();
    let ownerships = &block.execution_context().unwrap().lane_payload_ownerships;
    assert_eq!(ownerships.len(), 1);
    let proposal = proposal_from_ownership(&ownerships[0], block.hash()).unwrap();
    let session = committed_lane_session(&proposal, &keys[..3]);
    assert!(session.prepare_qc.payload_availability_qc.is_none());
    let pops = keys
        .iter()
        .take(3)
        .map(|key| {
            (
                key.public_key().clone(),
                iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap(),
            )
        })
        .collect();
    kura.persist_committed_lane_block_session(&session, &pops)
        .unwrap();
    assert!(
        kura.persist_lane_block_application_receipt_if_ready(&proposal)
            .unwrap()
    );
    let receipt = kura
        .read_lane_application_receipt(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::Current
    );
    assert!(
        state
            .certified_lane_block_session_is_applied_or_snapshot_anchored(&session)
            .unwrap()
    );
    let finality = kura.v2_finality_artifact(context.height).unwrap().unwrap();
    assert_eq!(finality.commit_qc.signers.len(), 3);
    let initial_state = crate::snapshot::canonical_state_snapshot_hash(state.as_ref()).unwrap();
    let initial_wire = block.encode_wire().unwrap();
    let mut limits = default_lane_work_test_limits();
    limits.session_capacity = NonZeroUsize::new(1).unwrap();
    limits.effect_capacity = NonZeroUsize::new(1).unwrap();
    let member_key = keys[0].clone();
    {
        let mut retiring = V2LaneWorkAdapter::new(
            context.clone(),
            PeerId::new(member_key.public_key().clone()),
            member_key.clone(),
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            Some(crate::sumeragi::v2_recovery::PendingKuraApply::for_test(
                context.id(),
                context.height,
                block.hash(),
            )),
        )
        .unwrap();
        retiring
            .lane_sessions
            .insert_proposal(proposal.clone())
            .unwrap();
        retiring
            .lane_sessions
            .insert_qc_with_pops(session.prepare_qc.clone(), &pops)
            .unwrap();
        let vote = signed_lane_vote(&proposal, CertPhase::Commit, &keys[0]);
        retiring.lane_sessions.insert_vote(vote, None).unwrap();
        retiring
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                session: session.clone(),
                next_validator: 1,
            });
        assert_eq!(retiring.lane_sessions.len(), 1);
        assert_eq!(
            retiring.committed_lane_outputs.len(),
            limits.session_capacity.get()
        );
        retiring.prepare_canonical_lane_rollover(&finality).unwrap();
        assert!(retiring.lane_sessions.is_empty());
        assert!(retiring.lane_sessions.rollover_slots().is_empty());
        assert_eq!(retiring.committed_lane_outputs.len(), 1);
        assert_eq!(retiring.committed_lane_outputs[0].session, session);
        assert_eq!(retiring.committed_lane_outputs[0].next_validator, 1);
        assert!(!retiring.output_guard.restart_required());
    }
    let observer_key = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).unwrap();
    let certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc: session.prepare_qc.clone(),
        commit_qc: session.commit_qc.clone(),
    };
    let sender = proposal.descriptor.validator_set[1].clone();
    for (key, voting) in [(member_key, true), (observer_key, false)] {
        let local_peer = PeerId::new(key.public_key().clone());
        assert_eq!(
            proposal.descriptor.validator_set.contains(&local_peer),
            voting
        );
        let mut adapter = V2LaneWorkAdapter::new(
            successor.clone(),
            local_peer,
            key,
            voting,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .unwrap();
        assert!(adapter.effects.is_empty());
        let first_recipient = (1..session.commit_qc.validator_set.len())
            .find(|index| session.commit_qc.validator_set[*index] != adapter.local_peer)
            .expect("retained committed output has an unsent remote recipient");
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                session: session.clone(),
                next_validator: 1,
            });
        let admit = |certificate| {
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                sender.clone(),
            ))
        };
        for _ in 0..2 {
            assert_eq!(
                adapter
                    .accept_lane_message_with_ingress_ownership(admit(certificate.clone()), 0)
                    .unwrap(),
                V2LaneIngressOutcome::Duplicate
            );
        }
        for phase in [CertPhase::Prepare, CertPhase::Commit] {
            let mut corrupt = certificate.clone();
            match phase {
                CertPhase::Prepare => corrupt.prepare_qc.bls_aggregate_signature[0] ^= 0x80,
                CertPhase::Commit => corrupt.commit_qc.bls_aggregate_signature[0] ^= 0x80,
                CertPhase::NewView => unreachable!(),
            }
            assert_eq!(
                adapter
                    .accept_lane_message_with_ingress_ownership(admit(corrupt), 0)
                    .unwrap(),
                V2LaneIngressOutcome::Rejected
            );
            assert!(adapter.sign_lane_vote(&proposal, phase).unwrap().is_none());
        }
        assert!(adapter.lane_sessions.is_empty());
        assert!(adapter.historical_recovery_sessions.is_empty());
        assert!(adapter.pending_committed_lanes.is_empty());
        assert!(adapter.lane_ready_authorizations.is_empty());
        // Duplicate ingress can drive the already-retained CommitQC owner.
        // One exact fanout fills the effect capacity; further ingress must
        // preserve its cursor instead of signing fresh work or losing custody.
        assert_eq!(adapter.effects.len(), limits.effect_capacity.get());
        let retained_outputs = adapter.drain_effects(usize::MAX);
        assert_eq!(retained_outputs.len(), 1);
        assert!(
            matches!(&retained_outputs[0], V2LaneWorkEffect::PostLaneBlock {
            peer, message: BlockMessage::LaneBlockQc(qc),
        } if peer == &session.commit_qc.validator_set[first_recipient]
            && qc == &session.commit_qc)
        );
        assert_eq!(adapter.committed_lane_outputs.len(), 1);
        assert_eq!(
            adapter.committed_lane_outputs[0].next_validator,
            first_recipient + 1
        );
        let mut routes = NetworkReplyRouteTestFixture::new(sender.clone());
        let route = routes.mint(sender.clone());
        let request = fair_v2_ingress_admit_for_test(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::LaneBlockProposal(proposal.clone()),
                sender.clone(),
                sender.clone(),
                route.clone(),
            )
            .unwrap(),
        );
        assert_eq!(
            adapter
                .accept_lane_message_with_ingress_ownership(request, 0)
                .unwrap(),
            V2LaneIngressOutcome::Inserted
        );
        adapter
            .purge_queued_global_body_effects_except_committed_outputs()
            .unwrap();
        let effects = adapter.drain_effects(usize::MAX);
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], V2LaneWorkEffect::PostDurableLaneCertificate {
            peer, reply_routes: Some(retained), ingress_ownership: Some(ownership), certificate: served,
        } if peer == &sender && served == &certificate && retained.len() == 1
            && retained.iter().any(|candidate| candidate.same_delivery(&route))
            && ownership.validate_exact() && ownership.matches_reply_routes(Some(retained)))
        );
        for changed_incarnation in [false, true] {
            let mut different = proposal.clone();
            if changed_incarnation {
                different.descriptor.lane_incarnation =
                    Hash::new(b"different completed incarnation");
            } else {
                different.descriptor.subject_hash = Hash::new(b"different completed subject");
            }
            different.descriptor.descriptor_hash = different.descriptor.computed_descriptor_hash();
            different.proposal_hash = different.computed_proposal_hash();
            validate_lane_block_proposal(&different).unwrap();
            assert!(
                adapter
                    .reconstruct_durable_lane_certificate(&different, &sender)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(adapter.lane_sessions.is_empty());
        assert!(adapter.effects.is_empty());
        assert_eq!(adapter.committed_lane_outputs.len(), 1);
        assert_eq!(adapter.committed_lane_outputs[0].session, session);
        assert_eq!(
            adapter.committed_lane_outputs[0].next_validator,
            first_recipient + 1
        );
        assert!(!adapter.output_guard.restart_required());
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(state.as_ref()).unwrap(),
            initial_state
        );
        assert_eq!(
            kura.get_block(NonZeroUsize::new(context.height as usize).unwrap())
                .unwrap()
                .encode_wire()
                .unwrap(),
            initial_wire
        );
        assert_eq!(
            kura.v2_finality_artifact(context.height).unwrap(),
            Some(finality.clone())
        );
        assert_eq!(
            kura.read_lane_application_receipt(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height
            )
            .unwrap(),
            Some(receipt.clone())
        );
    }
}
