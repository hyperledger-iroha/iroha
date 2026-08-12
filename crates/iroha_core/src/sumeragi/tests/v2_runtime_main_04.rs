#[test]
fn pre_timeout_scheduler_owner_may_publish_across_the_physical_snapshot() {
    let directory = TempDir::new().expect("temporary straddled-TV runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships(&context, &[], lifecycle_ordinals);
    assert!(ownerships.is_empty());

    let started_at = Instant::now();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the runtime before reproducing the admission straddle");
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("publish the empty receiver snapshot");
    let message = signed_runtime_timeout_vote(&context, &keys, 0, 1);
    let source = context.roster[1].validator.clone();
    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));

    let deadline = started_at + runtime.round_timeout();
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(deadline)
        .expect("freeze the timeout after the vote scheduler owner was reserved");
    let timeout_physical_cut = runtime
        .timeout_owner_physical_cut
        .expect("the timeout freezes the earlier empty receiver snapshot");
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("refresh the live receiver snapshot after physical publication");
    assert!(
        leader_wire_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.ingress_ownership().is_some_and(|ownership| {
                    runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
                })
            })
            .is_none(),
        "the vote cannot cross before TimeoutIntent owns its durable turn"
    );

    let timeout_step = runtime
        .step(deadline)
        .expect("the absolute timeout runs before the straddled vote");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("frozen timeout unexpectedly idled")
    };
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout publishes exact scheduler ownership");
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("durable TimeoutIntent transfers one signer owner");
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending timeout signer owner");

    let mut inbound = leader_wire_ingress
        .try_recv_if(|inbound| {
            let BlockMessage::V2(candidate) = inbound.message() else {
                return false;
            };
            inbound.ingress_ownership().is_some_and(|ownership| {
                runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
            })
        })
        .expect("the straddled vote descends after TimeoutIntent is durable");
    let ownership = inbound
        .take_ingress_ownership()
        .expect("checked dequeue retains exact straddled ownership");
    let token = ownership
        .leader_wire_token()
        .expect("the straddled vote owns one productive token");
    let physical_ordinal = ownership
        .physical_admission_ordinal()
        .expect("the straddled vote owns one physical carrier");
    assert_eq!(token.admission_ordinal(), physical_ordinal);
    assert!(u128::from(physical_ordinal) >= timeout_physical_cut);
    assert!(token.scheduler_ordinal() < timeout_owner.lifecycle_ordinal());
    let candidate = runtime
        .timeout_vote_recovery_candidate_from_fair(&message.payload, &ownership)
        .expect("the straddled clock-origin classification is exact")
        .expect("the current-view vote belongs to the finite episode");
    assert_eq!(
        candidate.owner.disposition,
        RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent
    );
    let plan = runtime
        .timeout_vote_episode_admission_plan(Some(candidate))
        .expect("the first straddled source slot is admissible");
    assert!(matches!(
        &plan,
        RuntimeTimeoutVoteEpisodeAdmissionPlan::FirstAdmission { .. }
    ));
    assert_eq!(plan.count_transition(), (0, 1));
    runtime
        .enqueue_network_with_ingress_ownership(message, ownership)
        .expect("authenticate and enqueue the straddled TimeoutVote");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);
    assert!(!runtime.fail_closed);
}

#[test]
fn two_fresh_timeout_vote_slots_replenish_once_and_close_a_four_validator_view() {
    let directory = TempDir::new().expect("temporary fresh-TV runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(4, 1, 1),
        Some(0),
    );
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships(&context, &[], lifecycle_ordinals);
    assert!(ownerships.is_empty());

    let started_at = Instant::now();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the four-validator runtime");
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("publish the empty fair-ingress cut before timeout");
    let deadline = started_at + runtime.round_timeout();
    let timeout_step = runtime
        .step(deadline)
        .expect("the local absolute timeout opens its finite TV episode");
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("absolute timeout unexpectedly idled")
    };
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout Sign retains its exact lifecycle owner");
    let [timeout_effect_owner] = timeout_effect_ownership.as_slice() else {
        panic!("timeout emits one exact signing effect")
    };
    let (signature_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_owner.owner().clone()])
        .expect("publish the pending local timeout signer");
    let timeout_ordinal = runtime
        .timeout_recovery_episode
        .as_ref()
        .expect("durable timeout retains its finite episode")
        .timeout_owner
        .lifecycle_ordinal();
    assert_eq!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("timeout episode remains active")
            .timeout_vote_owner_universe
            .len(),
        context.roster.len(),
        "the frozen producer universe has one TimeoutVote slot per roster source"
    );
    let frozen_universe = runtime
        .timeout_recovery_episode
        .as_ref()
        .expect("timeout episode retains its roster universe")
        .timeout_vote_owner_universe
        .clone();
    let _removed_slot = runtime
        .timeout_recovery_episode
        .as_mut()
        .expect("timeout episode remains mutable only inside this negative test")
        .timeout_vote_owner_universe
        .pop_first()
        .expect("four-validator universe is non-empty");
    assert!(
        runtime.timeout_recovery_lifecycle_cut().is_err(),
        "a changed frozen roster universe must invalidate the episode"
    );
    runtime
        .timeout_recovery_episode
        .as_mut()
        .expect("restore the exact test episode")
        .timeout_vote_owner_universe = frozen_universe;
    assert_eq!(
        runtime
            .timeout_recovery_lifecycle_cut()
            .expect("the restored roster universe validates"),
        Some(timeout_ordinal)
    );

    for signer in [1_u32, 2_u32] {
        let highest_prepare_qc = (signer == 2).then(|| {
            signed_runtime_quorum_certificate_for_phase(
                &context,
                &keys,
                0xD2,
                wire::GlobalPhase::Prepare,
            )
        });
        let message = signed_runtime_timeout_vote_with_highest_prepare_qc(
            &context,
            &keys,
            0,
            signer,
            highest_prepare_qc,
        );
        let source = context.roster[usize::try_from(signer).expect("small signer index")]
            .validator
            .clone();
        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message.clone()),
                Some(source.clone()),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
        let mut inbound = leader_wire_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.ingress_ownership().is_some_and(|ownership| {
                    runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
                })
            })
            .expect("one fresh roster slot crosses the finite TV episode");
        let ownership = inbound
            .take_ingress_ownership()
            .expect("checked dequeue retains exact fresh-TV ownership");
        let token = ownership
            .leader_wire_token()
            .expect("fresh TimeoutVote owns one productive token")
            .clone();
        let physical_ordinal = ownership
            .physical_admission_ordinal()
            .expect("fresh TimeoutVote owns its first physical carrier");
        assert_eq!(token.admission_ordinal(), physical_ordinal);
        assert!(token.scheduler_ordinal() > timeout_ordinal);
        let candidate = runtime
            .timeout_vote_recovery_candidate_from_fair(&message.payload, &ownership)
            .expect("fresh candidate classification is exact")
            .expect("fresh current-view TV belongs to the episode");
        let first_plan = runtime
            .timeout_vote_episode_admission_plan(Some(candidate.clone()))
            .expect("first source-slot plan is valid");
        assert!(matches!(
            &first_plan,
            RuntimeTimeoutVoteEpisodeAdmissionPlan::FirstAdmission { .. }
        ));
        assert_eq!(first_plan.count_transition(), (0, 1));

        runtime
            .enqueue_network_with_ingress_ownership(message.clone(), ownership)
            .expect("authenticate and admit the fresh TimeoutVote");
        let coalesced_plan = runtime
            .timeout_vote_episode_admission_plan(Some(candidate.clone()))
            .expect("the exact incumbent slot remains valid");
        assert!(matches!(
            &coalesced_plan,
            RuntimeTimeoutVoteEpisodeAdmissionPlan::CoalescedRetry { .. }
        ));
        assert_eq!(coalesced_plan.count_transition(), (1, 1));
        let episode = runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("fresh admission keeps the episode active");
        assert_eq!(
            episode.admitted_timeout_vote_owners[&token.slot].disposition,
            RuntimeTimeoutVoteEpisodeDisposition::FreshReplenishment
        );
        assert_eq!(
            token.identity.subject_hash,
            Hash::new([]),
            "TimeoutVote lifecycle identity excludes the carried highest-QC subject"
        );
        assert_eq!(
            episode.admitted_timeout_vote_owners.len(),
            usize::try_from(signer).expect("small signer count"),
            "each distinct roster source increases the finite count once"
        );
        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::new(
                BlockMessage::V2(message),
                Some(source),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Coalesced)
        ));
        assert_eq!(
            runtime
                .timeout_recovery_episode
                .as_ref()
                .expect("exact retry cannot retire the episode")
                .admitted_timeout_vote_owners
                .len(),
            usize::try_from(signer).expect("small signer count"),
            "an exact retry is 1→1 rather than replenishment"
        );

        let queue_len_before_replacement = runtime.queued_commands();
        let owners_before_replacement = runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("episode retains its exact source map")
            .admitted_timeout_vote_owners
            .clone();
        let mut replaced_token = candidate.clone();
        replaced_token.owner.token.identity.canonical_wire_hash =
            Hash::new([0xA0, u8::try_from(signer).expect("small signer marker")]);
        assert_eq!(
            runtime.timeout_vote_episode_admission_plan(Some(replaced_token)),
            Err(EnqueueError::FailClosed),
            "a different token cannot replace the occupied source slot"
        );
        let mut malformed_carrier = candidate.clone();
        malformed_carrier.owner.carrier_physical_ordinal = malformed_carrier
            .owner
            .carrier_physical_ordinal
            .checked_add(1)
            .expect("small physical carrier has a successor");
        assert_eq!(
            runtime.timeout_vote_episode_admission_plan(Some(malformed_carrier)),
            Err(EnqueueError::FailClosed),
            "a fresh-replenishment disposition cannot claim mismatched carrier geometry"
        );
        assert_eq!(runtime.queued_commands(), queue_len_before_replacement);
        assert_eq!(
            runtime
                .timeout_recovery_episode
                .as_ref()
                .expect("rejected replacement cannot retire the episode")
                .admitted_timeout_vote_owners,
            owners_before_replacement,
            "replacement is rejected before queue or episode refinement"
        );
    }
    assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);
    let third_message = signed_runtime_timeout_vote(&context, &keys, 0, 3);
    let third_source = context.roster[3].validator.clone();
    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(third_message),
            Some(third_source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let owners_before_full = runtime
        .timeout_recovery_episode
        .as_ref()
        .expect("episode remains active at Progress capacity")
        .admitted_timeout_vote_owners
        .clone();
    assert!(
        leader_wire_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.ingress_ownership().is_some_and(|ownership| {
                    runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
                })
            })
            .is_none(),
        "a third fresh source remains physically owned while Progress capacity is full"
    );
    assert_eq!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("capacity backpressure cannot retire the episode")
            .admitted_timeout_vote_owners,
        owners_before_full,
        "queue-full preflight cannot publish a 0→1 episode refinement"
    );

    let local_signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(signature_tag, local_signature, timeout_effect_owner)
        .expect("enqueue the local timeout signature at the inclusive timeout cut");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the external signer after queuing its completion");

    let mut saw_local_timeout_vote = false;
    let mut saw_timeout_certificate = false;
    let mut saw_enter_view = false;
    for _ in 0..12 {
        let step = runtime
            .try_step_pacemaker_escape(deadline)
            .expect("finite TV episode keeps the typed pacemaker live")
            .expect("local completion or one admitted TV remains runnable");
        runtime
            .take_last_scheduler_ownership()
            .expect("every pacemaker step publishes exact scheduler ownership");
        let RuntimeStep::Advanced(effects) = step else {
            panic!("typed pacemaker episode unexpectedly idled")
        };
        saw_local_timeout_vote |= effects.iter().any(|effect| {
            matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::TimeoutVote(_)
                    )
            )
        });
        saw_timeout_certificate |= effects.iter().any(|effect| {
            matches!(
                effect,
                AdapterEffect::Broadcast(message)
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
                    )
            )
        });
        saw_enter_view |= effects.iter().any(
            |effect| matches!(effect, AdapterEffect::EnterView { tag, .. } if tag.view() == 1),
        );
        runtime
            .take_effect_ownership(effects.len())
            .expect("consume the exact effect ownership for this macro-step");
        let _ = runtime.take_leader_wire_runtime_terminals();
        if runtime.round_tag().view() == 1 {
            break;
        }
    }
    assert!(saw_local_timeout_vote);
    assert!(saw_timeout_certificate);
    assert!(saw_enter_view);
    assert_eq!(runtime.round_tag().view(), 1);
    assert!(runtime.timeout_recovery_episode.is_none());
    assert!(!runtime.fail_closed);
}

#[test]
fn restored_timeout_vote_reactivation_binds_fresh_carrier_before_runtime_admission() {
    let runtime_directory = TempDir::new().expect("temporary restored-TV runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &runtime_directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let message = signed_runtime_timeout_vote(&context, &keys, 0, 1);
    let source = context.roster[1].validator.clone();
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (leader_wire_directory, first_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), source.clone())],
        lifecycle_ordinals.clone(),
    );
    let [first_runtime_ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("first process creates one exact Runtime owner");
    let first_receipt = first_runtime_ownership
        .leader_wire_runtime_receipt()
        .expect("first process durably binds Runtime ownership")
        .clone();
    let token = first_receipt.token().clone();
    first_ingress.close();
    drop(first_ingress);

    let restored_ingress = Arc::new(
        super::super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    restored_ingress
        .configure_roster_for_context(roster.clone(), &context.network_id, context.da_layout)
        .expect("restored leader-wire geometry");
    restored_ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            context.da_layout.max_chunk_count,
        )
        .expect("finite restored leader-wire capacity");
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            [0xE7; 32],
            0,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &leader_wire_directory
                .path()
                .join("leader-wire-preowned.wal"),
            context.id(),
            context.height,
            [0xE7; 32],
            roster.iter().cloned().collect(),
            capacity,
            context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("reopen the same durable leader-wire lifecycle");
    assert_eq!(restore.records().len(), 1);
    assert_eq!(restore.records()[0].token(), &token);
    assert_eq!(
        restore.records()[0].status(),
        super::super::serviced_candidate_store::LeaderWireLifecycleStatus::Dormant
    );
    assert_eq!(
        restore.scheduler_ordinal_high_watermark(),
        token.scheduler_ordinal()
    );
    restored_ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            lifecycle_ordinals,
            context.id(),
            context.height,
        )
        .expect("bind restored leader-wire state and advance shared high-watermark");
    restored_ingress
        .open()
        .expect("open restored fair ingress only after binding");

    let started_at = Instant::now();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime");
    runtime
        .set_ingress_physical_cut(restored_ingress.next_physical_admission_ordinal())
        .expect("publish the restored selector high-watermark before freezing timeout");
    let deadline = started_at + runtime.round_timeout();
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(deadline)
        .expect("freeze the post-restart absolute timeout owner");
    assert!(token.scheduler_ordinal() < timeout_owner.lifecycle_ordinal());
    let timeout_cut = runtime
        .timeout_owner_physical_cut
        .expect("timeout freezes the restored receiver cut");

    assert!(matches!(
        restored_ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(source),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    assert!(
        restored_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.ingress_ownership().is_some_and(|ownership| {
                    runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
                })
            })
            .is_none(),
        "the restored carrier remains queued until TimeoutIntent is durable"
    );

    let timeout_step = runtime
        .step(deadline)
        .expect("absolute timeout dispatches before the restored handoff");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("frozen timeout unexpectedly idled")
    };
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("persisted TimeoutIntent transfers one signer owner");
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending timeout signer owner");

    let mut replay = restored_ingress
        .try_recv_if(|inbound| {
            let BlockMessage::V2(candidate) = inbound.message() else {
                return false;
            };
            inbound.ingress_ownership().is_some_and(|ownership| {
                runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
            })
        })
        .expect("checked dequeue admits the exact strict TimeoutVote replay");
    let replay_ownership = replay
        .take_ingress_ownership()
        .expect("checked dequeue retains exact ownership");
    assert_eq!(replay_ownership.leader_wire_token(), Some(&token));
    assert_eq!(
        replay_ownership.leader_wire_runtime_receipt(),
        Some(&first_receipt),
        "restart reuses the immutable Runtime owner rather than replacing it"
    );
    let replay_physical_ordinal = replay_ownership
        .physical_admission_ordinal()
        .expect("reactivation owns one fresh physical carrier");
    assert!(token.admission_ordinal() < replay_physical_ordinal);
    assert!(u128::from(replay_physical_ordinal) >= timeout_cut);
    assert!(
        replay_ownership
            .runtime_physical_cut()
            .is_some_and(|cut| cut > u128::from(replay_physical_ordinal))
    );
    runtime
        .enqueue_network_with_ingress_ownership(message, replay_ownership)
        .expect("authenticated atomic handoff enters ordinary Progress capacity");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);
    assert!(!runtime.fail_closed);
}

#[test]
fn exact_authenticated_qc_from_distinct_sources_coalesces_in_one_runtime_slot() {
    let directory = TempDir::new().expect("temporary multi-source QC directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let owner_tag = runtime.round_tag();
    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC7);
    let message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
    );
    let first_source = PeerId::new(keys[0].public_key().clone());
    let second_source = PeerId::new(keys[1].public_key().clone());

    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, first_source),
            )
            .expect("the first authenticated carrier owns the runtime command"),
        owner_tag
    );
    assert_eq!(
        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, second_source),
            )
            .expect("an exact QC from another source coalesces"),
        owner_tag
    );
    assert_eq!(runtime.queued_commands(), 1);

    let retained = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("the queued QC retains fair-ingress ownership");
    assert!(retained.validate_exact());
    assert_eq!(retained.direct.len(), 2);
    assert!(retained.commit_certificate_response.is_empty());
    assert_ne!(
        retained.direct[0].process_local_projection_hash(),
        retained.direct[1].process_local_projection_hash(),
        "direct carrier projections must retain their distinct authenticated-source identities"
    );

    let mut source_substituted = retained.clone();
    let substituted_source = PeerId::from(KeyPair::random().public_key().clone());
    source_substituted.direct[0].first.wire_key.origin = Some(substituted_source.clone());
    source_substituted.direct[0].first.semantic_origin = Some(substituted_source.clone());
    source_substituted.direct[0].first.authenticated_via = Some(substituted_source.clone());
    source_substituted.direct[0].first.authenticated_source =
        super::super::FairV2IngressSource::Validator(substituted_source.clone());
    source_substituted.direct[0].first.semantic_owner_source =
        super::super::FairV2IngressSource::Validator(substituted_source.clone());
    source_substituted.direct[0].latest.wire_key.origin = Some(substituted_source.clone());
    source_substituted.direct[0].latest.semantic_origin = Some(substituted_source.clone());
    source_substituted.direct[0].latest.authenticated_via = Some(substituted_source.clone());
    source_substituted.direct[0].latest.authenticated_source =
        super::super::FairV2IngressSource::Validator(substituted_source.clone());
    source_substituted.direct[0].latest.semantic_owner_source =
        super::super::FairV2IngressSource::Validator(substituted_source);
    assert!(source_substituted.direct[0].validate_exact());
    assert!(
        !source_substituted.validate_exact(),
        "the retained runtime projection must reject an otherwise exact source substitution"
    );

    let mut reordered = retained.clone();
    reordered.direct.reverse();
    assert!(
        !reordered.validate_exact(),
        "the retained runtime projection must reject carrier-order mutation"
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn exact_authenticated_tc_from_distinct_sources_bypasses_signer_as_one_owner() {
    let directory = TempDir::new().expect("temporary multi-source TC directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(4, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before authenticated ingress");
    let owner_tag = runtime.round_tag();
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
            signed_runtime_timeout_certificate(&context, &keys),
        ));
    let deadline = now + runtime.round_timeout();
    let timeout_step = runtime
        .step(deadline)
        .expect("install a runtime-owned local signing fence");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout dispatch retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("timeout dispatch unexpectedly idled")
    };
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout Sign retains its lifecycle owner");
    assert_eq!(timeout_effect_ownership.len(), 1);
    match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            },
        ] => {}
        effects => panic!("unexpected timeout effects: {effects:?}"),
    }
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
        .expect("publish the pending timeout signer owner");

    for source in &keys[..2] {
        assert_eq!(
            runtime
                .enqueue_network_with_ingress_ownership(
                    message.clone(),
                    fair_network_ownership(&message, PeerId::new(source.public_key().clone()),),
                )
                .expect("each authenticated TC carrier coalesces"),
            owner_tag
        );
    }
    assert_eq!(runtime.queued_commands(), 1);
    let queued = runtime
        .ingress
        .commands
        .front()
        .and_then(|command| command.ingress_ownership.as_ref())
        .expect("the queued TC retains both fair-ingress carriers");
    assert_eq!(queued.direct.len(), 2);
    assert!(queued.validate_exact());

    let step = runtime
        .try_step_pacemaker_escape(deadline)
        .expect("certified pacemaker selection remains valid")
        .expect("the queued TC owns one typed pacemaker turn");
    let RuntimeStep::Advanced(effects) = step else {
        panic!("certified TC unexpectedly idled")
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::EnterView { tag, .. }] if tag.view() == owner_tag.view() + 1
    ));
    let fifo_owner = runtime
        .take_last_scheduler_ownership()
        .expect("certified TC dispatch retains its exact FIFO owner");
    assert!(fifo_owner.validate_exact().is_ok());
    assert_eq!(
        fifo_owner.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &fifo_owner.candidate else {
        panic!("certified TC must retain its exact queued candidate")
    };
    assert_eq!(
        candidate.selection_seal.kind,
        RuntimeQueueSelectionKind::PacemakerCertifiedProgress
    );
    assert!(
        candidate
            .ingress_ownership
            .as_ref()
            .is_some_and(|ownership| { ownership.validate_exact() && ownership.direct.len() == 2 })
    );
    runtime
        .take_effect_ownership(effects.len())
        .expect("the executor consumes the TC EnterView owner");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("the executor retires the superseded signer owner");
    assert_eq!(runtime.queued_commands(), 0);
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(!runtime.fail_closed);
}

#[test]
fn same_semantic_qc_with_conflicting_route_authority_fails_closed_atomically() {
    let directory = TempDir::new().expect("temporary conflicting route directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC8);
    let message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
    );
    let source = PeerId::new(keys[0].public_key().clone());
    let mut routes = NetworkReplyRouteTestFixture::new(source.clone());
    let first_route = routes.mint(source.clone());
    let conflicting_route = routes
        .forge_equal_ordinal_different_tenure(&first_route, source.clone(), source.clone())
        .expect("fixture owns the conflicting route authority");

    assert!(matches!(
        super::super::InboundBlockMessage::try_from_transport_with_reply_route(
            super::super::message::BlockMessage::V2(message.clone()),
            source.clone(),
            source.clone(),
            conflicting_route.clone(),
        ),
        Err(NetworkReplyRouteError::EqualOrdinalDifferentTenure)
    ));
    let first_ownership =
        fair_network_ownership_with_route(&message, source.clone(), source.clone(), first_route);
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), first_ownership.clone())
        .expect("the first exact route owns the authenticated QC");
    let retained_before = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("the queued QC retains its first route")
        .clone();

    let mut conflicting_ownership = retained_before.direct[0].clone();
    conflicting_ownership.attempts[0].route = conflicting_route.clone();
    conflicting_ownership.latest.attempts_after[0].route = conflicting_route;
    assert!(
        !conflicting_ownership.validate_exact(),
        "the runtime must reject a carrier whose cursor projection substitutes a forged tenure"
    );
    assert!(matches!(
        runtime.enqueue_network_with_ingress_ownership(message.clone(), conflicting_ownership),
        Err(NetworkIngressError::FailClosed)
    ));
    let retained_after = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("failed merge preserves the first exact route");
    assert_eq!(retained_after, &retained_before);
    assert_eq!(retained_after.direct.len(), 1);
    assert_eq!(
        runtime.fail_closed_reason.as_deref(),
        Some("network ingress changed its authenticated fair-queue ownership")
    );
}

#[test]
fn runtime_ingress_carrier_capacity_returns_backpressure_atomically() {
    let directory = TempDir::new().expect("temporary carrier-capacity directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
    let certificate = signed_runtime_quorum_certificate(&context, &keys, 0xC9);
    let message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
    );
    let carrier = || {
        let source = PeerId::from(KeyPair::random().public_key().clone());
        fair_network_ownership(&message, source)
    };
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), carrier())
        .expect("the first disjoint carrier owns the authenticated QC");
    for _ in 1..MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM {
        let candidate = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, carrier())
            .expect("independent fair-ingress carrier is exact");
        runtime
            .ingress
            .commands
            .front_mut()
            .and_then(|queued| queued.ingress_ownership.as_mut())
            .expect("the queued QC retains its carrier set")
            .merge_downstream(candidate)
            .expect("every protocol-bounded carrier remains exact");
    }
    let retained = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("the queued QC retains the full carrier set");
    assert_eq!(retained.direct.len(), MAX_RUNTIME_INGRESS_CARRIERS_PER_FORM);
    let retained_before = retained.clone();
    let queued_before = runtime.queued_commands();
    let excess_carrier = carrier();

    assert!(matches!(
        runtime.enqueue_network_with_ingress_ownership(message, excess_carrier),
        Err(NetworkIngressError::Backpressure(EnqueueError::Full))
    ));
    let retained_after = runtime
        .ingress
        .commands
        .front()
        .and_then(|queued| queued.ingress_ownership.as_ref())
        .expect("backpressure preserves the full exact carrier set");
    assert_eq!(retained_after, &retained_before);
    assert_eq!(
        runtime.queued_commands(),
        queued_before,
        "carrier saturation must not create a duplicate runtime command"
    );
    assert!(retained_after.validate_exact());
    assert!(!runtime.fail_closed);
    assert!(runtime.fail_closed_reason.is_none());
}

#[test]
fn exact_authenticated_retransmission_preserves_capacity_fifo_and_cursor() {
    let round = wire::ConsensusRound {
        context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"coalesced-capacity-context",
        ))),
        height: 9,
        view: 4,
    };
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"coalesced-capacity-block")),
        payload_hash: Hash::new(b"coalesced-capacity-payload"),
    };
    let payload = |signature| {
        wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"capacity parent state"),
                Hash::new(b"capacity post state"),
                Hash::new(b"capacity ordinary writes"),
                1,
                Hash::new(b"capacity executed block wire"),
            ),
            signer: 0,
            signature: vec![signature],
        })
    };
    let authenticated = |signature| {
        AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(payload(signature)))
    };
    let queued_wire = wire::ConsensusMessageV2::new(payload(1));
    let transport = wire::ConsensusMessageV2Payload::PayloadManifest(wire::PayloadManifest {
        round,
        subject,
        payload_size_bytes: 1,
        layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 2,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 1,
            max_chunk_count: 2,
        },
        chunk_hashes: vec![Hash::new(b"coalesced capacity chunk"); 2],
        chunk_root: Hash::new(b"coalesced capacity root"),
    });
    assert!(matches!(
        classify_reducer_network_ingress(false, &queued_wire.payload),
        Ok(CommandClass::Normal)
    ));
    assert!(matches!(
        classify_reducer_network_ingress(false, &transport),
        Err(NetworkIngressError::TransportPayload)
    ));
    assert!(matches!(
        classify_reducer_network_ingress(true, &queued_wire.payload),
        Err(NetworkIngressError::FailClosed)
    ));
    assert!(matches!(
        classify_reducer_network_ingress(true, &transport),
        Err(NetworkIngressError::FailClosed)
    ));
    let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(5, 1, 1));

    assert_eq!(
        ingress
            .enqueue_authenticated(tag(0), CommandClass::Normal, authenticated(1))
            .expect("first wire value enters below the normal boundary"),
        tag(0)
    );
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(1), CommandClass::Normal, authenticated(2))
            .expect("a non-identical wire value uses ordinary capacity"),
        tag(1)
    );
    assert_eq!(
        ingress.check_capacity(CommandClass::Normal),
        Err(EnqueueError::ReservedCapacity)
    );

    let cursor_before = ingress.next_class;
    let tags_before = ingress
        .commands
        .iter()
        .map(|queued| queued.tag)
        .collect::<Vec<_>>();
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(8), CommandClass::Normal, authenticated(1))
            .expect("an exact duplicate coalesces at reserved capacity"),
        tag(0),
        "coalescing deterministically returns the original admission tag"
    );
    assert_eq!(ingress.next_class, cursor_before);
    assert_eq!(
        ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>(),
        tags_before,
        "coalescing changes neither FIFO ownership nor its tags"
    );
    assert_eq!(
        ingress.enqueue_authenticated(tag(9), CommandClass::Normal, authenticated(3)),
        Err(EnqueueError::ReservedCapacity),
        "a non-identical envelope still obeys the normal boundary"
    );

    ingress
        .enqueue_authenticated(tag(2), CommandClass::Progress, authenticated(3))
        .expect("progress reserve remains independent");
    ingress
        .enqueue_authenticated(tag(3), CommandClass::Completion, authenticated(4))
        .expect("completion reserve fills the final ordinary slot");
    assert_eq!(ingress.len(), 4);
    assert_eq!(
        ingress.check_capacity(CommandClass::Completion),
        Err(EnqueueError::Full)
    );
    assert_eq!(ingress.authenticated_wire_tag(&queued_wire), Some(tag(0)));
    assert!(
        ingress
            .check_authenticated_wire_capacity(&queued_wire, CommandClass::Normal, false,)
            .is_ok(),
        "raw equality only opens the authentication attempt at full capacity"
    );
    assert_eq!(
        ingress.check_authenticated_wire_capacity(
            &wire::ConsensusMessageV2::new(payload(5)),
            CommandClass::Normal,
            false,
        ),
        Err(EnqueueError::Full)
    );

    let full_tags = ingress
        .commands
        .iter()
        .map(|queued| queued.tag)
        .collect::<Vec<_>>();
    assert_eq!(
        ingress
            .enqueue_authenticated(tag(10), CommandClass::Normal, authenticated(1))
            .expect("the exact envelope coalesces when every ordinary slot is owned"),
        tag(0)
    );
    assert_eq!(ingress.next_class, cursor_before);
    assert_eq!(
        ingress
            .commands
            .iter()
            .map(|queued| queued.tag)
            .collect::<Vec<_>>(),
        full_tags
    );
    assert!(
        ingress
            .commands
            .iter()
            .all(|queued| queued.eligible_skips == 0)
    );
    assert_eq!(
        ingress.enqueue_authenticated(tag(11), CommandClass::Progress, authenticated(5)),
        Err(EnqueueError::Full),
        "wire inequality cannot inherit the duplicate's full-queue exception"
    );
}

#[test]
fn completion_retries_coalesce_across_ingress_and_busy_deferred_ownership() {
    let directory = TempDir::new().expect("temporary completion-coalescing directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let owner_tag = runtime.round_tag();
    let receipts = |manifest: &wire::PayloadManifest| {
        let durable = DurableBodyReceipt::for_test(
            context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(manifest),
        );
        let validated = ValidatedBodyReceipt::for_test(durable.clone());
        (durable, validated)
    };

    let ingress_manifest = runtime_manifest(&context, 0x91);
    let (durable, _) = receipts(&ingress_manifest);
    stage_completion_for_queue_test(
        &mut runtime,
        owner_tag,
        AdapterCommand::BodyStored {
            round: ingress_manifest.round,
            subject: ingress_manifest.subject,
            receipt: durable.clone(),
        },
    );
    runtime
        .enqueue_body_stored(
            owner_tag,
            ingress_manifest.round,
            ingress_manifest.subject,
            durable,
        )
        .expect("an exact retransmission coalesces in runtime ingress");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                ingress_manifest.round,
                ingress_manifest.subject,
            )
            .expect("retire the one coalesced ingress owner"),
        RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 0,
            local_proposal: 0,
        }
    );

    let deferred_store = runtime_manifest(&context, 0x92);
    let (durable, _) = receipts(&deferred_store);
    let active_before_store = runtime.driver.all_deferred_admission_ordinals();
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_store,
            DeferredBodyPipelineStageForTest::BodyStored,
        )
        .expect("stage a Busy-deferred durable-store completion");
    let store_ordinals = runtime
        .driver
        .all_deferred_admission_ordinals()
        .difference(&active_before_store)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(store_ordinals.len(), 1);
    let store_owner = bind_local_deferred_lifecycle_for_test(
        &mut runtime,
        store_ordinals[0],
        b"body-store-pipeline-retirement-owner",
    );
    runtime
        .enqueue_body_stored(
            owner_tag,
            deferred_store.round,
            deferred_store.subject,
            durable,
        )
        .expect("a retransmit coalesces with the Busy-deferred store owner");
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal()
            .expect("inspect the exact Busy-deferred store owner"),
        Some(store_owner.lifecycle_ordinal())
    );
    assert_eq!(
        runtime
            .retire_body_pipeline_completions(
                owner_tag,
                deferred_store.round,
                deferred_store.subject,
            )
            .expect("retire the coalesced Busy-deferred store owner"),
        RetiredBodyPipelineCompletions {
            body_available: 0,
            body_stored: 1,
            validation: 0,
            local_proposal: 0,
        }
    );
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert_eq!(
        runtime
            .minimum_active_lifecycle_ordinal()
            .expect("retirement cannot retain a phantom store owner"),
        None
    );

    let deferred_validation = runtime_manifest(&context, 0x93);
    let (_, validated) = receipts(&deferred_validation);
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_validation,
            DeferredBodyPipelineStageForTest::ValidationSucceeded,
        )
        .expect("stage a Busy-deferred validation completion");
    runtime
        .enqueue_validation_succeeded(
            owner_tag,
            deferred_validation.round,
            deferred_validation.subject,
            validated,
        )
        .expect("a retransmit coalesces with the Busy-deferred validation owner");
    assert_eq!(runtime.queued_commands(), 0);
    runtime
        .retire_body_pipeline_completions(
            owner_tag,
            deferred_validation.round,
            deferred_validation.subject,
        )
        .expect("retire the coalesced Busy-deferred validation owner");

    let deferred_proposal = runtime_manifest(&context, 0x94);
    let (durable, validated) = receipts(&deferred_proposal);
    runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            owner_tag,
            &deferred_proposal,
            DeferredBodyPipelineStageForTest::LocalProposalReady,
        )
        .expect("stage a Busy-deferred local-proposal completion");
    runtime
        .enqueue_local_proposal(owner_tag, deferred_proposal.clone(), durable, validated)
        .expect("a retransmit coalesces with the Busy-deferred proposal owner");
    assert_eq!(runtime.queued_commands(), 0);
    runtime
        .retire_body_pipeline_completions(
            owner_tag,
            deferred_proposal.round,
            deferred_proposal.subject,
        )
        .expect("retire the coalesced Busy-deferred proposal owner");
}

#[test]
fn body_available_rebind_rejects_uninstalled_destination_without_mutation() {
    let directory = TempDir::new().expect("temporary uninstalled-rebind directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let source_tag = runtime.round_tag();
    let fabricated = EventTag::new(
        source_tag.height(),
        source_tag.view() + 1,
        Generation::new(source_tag.generation().get() + 1),
    );
    let manifest = runtime_manifest(&context, 0x8B);
    runtime
        .enqueue_body_available(source_tag, manifest.clone())
        .expect("enqueue unique source owner");

    assert_eq!(
        runtime
            .rebind_body_available(source_tag, fabricated, &manifest)
            .expect_err("an uninstalled destination tag must be rejected"),
        "Sumeragi v2 body completion rebind target is not the installed runtime incarnation"
    );
    assert!(
        !runtime.fail_closed,
        "caller contract rejection is recoverable"
    );
    assert_eq!(runtime.round_tag(), source_tag);
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.ingress.commands.front(),
        Some(TaggedCommand {
            tag,
            command: AdapterCommand::BodyAvailable {
                manifest: queued_manifest,
            },
            ..
        }) if *tag == source_tag && queued_manifest == &manifest
    ));
    assert!(
        runtime
            .retire_body_available(source_tag, &manifest)
            .expect("the untouched source owner remains retireable")
    );
    assert_eq!(runtime.queued_commands(), 0);
}

#[test]
fn periodic_decision_store_retry_carries_durable_commit_authority() {
    let directory = TempDir::new().expect("temporary periodic Decision-store directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for production dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8C))
        .expect("enqueue authenticated proposal");
    let RuntimeStep::Advanced(proposal_effects) = runtime.step(now).expect("dispatch proposal")
    else {
        panic!("proposal dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("proposal dispatch publishes scheduler ownership");
    let (tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let proposal_ownership = runtime
        .take_effect_ownership(proposal_effects.len())
        .expect("FetchBody retains the proposal lifecycle owner");
    let _proposal_terminals = runtime.take_leader_wire_runtime_terminals();
    let reservation = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &proposal_ownership[0])
        .expect("reserve reconstructed body under its FetchBody owner");
    runtime
        .commit_body_available(reservation)
        .expect("publish the owned body reconstruction completion");

    let RuntimeStep::Advanced(store_effects) =
        runtime.step(now).expect("dispatch body reconstruction")
    else {
        panic!("body reconstruction unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("body reconstruction publishes scheduler ownership");
    assert!(matches!(
        store_effects.as_slice(),
        [AdapterEffect::StoreBody {
            round,
            subject,
            ..
        }] if *round == manifest.round && *subject == manifest.subject
    ));
    let incumbent_store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("ordinary StoreBody retains its FetchBody owner")
        .pop()
        .expect("one StoreBody has one owner");
    let incumbent_statement = incumbent_store_ownership
        .candidate_semantic_statement()
        .expect("ordinary StoreBody retains an exact body statement");
    assert_eq!(incumbent_statement.phase, None);
    assert_eq!(incumbent_statement.execution_commitment, None);
    runtime
        .set_external_lifecycle_owners(vec![incumbent_store_ownership.owner().clone()])
        .expect("publish the in-flight StoreBody owner to scheduler arbitration");
    assert_eq!(
        runtime
            .driver
            .body_state_for_test(manifest.round, manifest.subject),
        super::super::v2_core::BodyState::Available,
    );

    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable);
    runtime
        .bind_validated_body(&manifest, &validated)
        .expect("register the exact deterministic execution commitment");
    let decision = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x8D; 96],
    };
    runtime
        .ingress
        .enqueue_authenticated(
            tag,
            CommandClass::Progress,
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(decision.clone()),
            )),
        )
        .expect("enqueue the authenticated CommitQC");
    let RuntimeStep::Advanced(decision_effects) =
        runtime.step(now).expect("install the durable Decision")
    else {
        panic!("CommitQC dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("Decision dispatch publishes scheduler ownership");
    assert!(
        decision_effects.is_empty(),
        "an in-flight StoreBody remains the sole physical body task"
    );
    runtime
        .take_effect_ownership(decision_effects.len())
        .expect("empty Decision batch has no retained effect sidecar");
    let _decision_terminals = runtime.take_leader_wire_runtime_terminals();
    runtime
        .retire_proposal_work_after_decision(
            decision.proposal_round,
            decision.subject,
            decision.execution_commitment,
        )
        .expect("outer Decision reconciliation preserves body recovery");

    let periodic_at = now + runtime.retransmit_interval();
    let RuntimeStep::Advanced(recovery_effects) = runtime
        .step(periodic_at)
        .expect("periodic durable-Decision recovery advances")
    else {
        panic!("periodic durable-Decision recovery unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic recovery publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer,
    );
    let recovery_store_positions = recovery_effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            matches!(
                effect,
                AdapterEffect::StoreBody { round, subject, .. }
                    if *round == manifest.round && *subject == manifest.subject
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        recovery_store_positions.len(),
        1,
        "periodic Decision recovery must emit one exact StoreBody: {recovery_effects:?}"
    );
    assert!(recovery_effects.iter().enumerate().all(|(index, effect)| {
        recovery_store_positions.contains(&index) || matches!(effect, AdapterEffect::Broadcast(_))
    }));
    let recovery_ownership = runtime
        .take_effect_ownership(recovery_effects.len())
        .expect("periodic StoreBody retains exact durable authority")
        .swap_remove(recovery_store_positions[0]);
    assert_ne!(
        recovery_ownership.owner(),
        incumbent_store_ownership.owner(),
        "the periodic producer remains a distinct physical retry root"
    );
    assert!(recovery_ownership.binds_durable_decision_authority(
        decision.round,
        decision.proposal_round,
        decision.subject,
        decision.execution_commitment,
    ));
    assert!(!runtime.fail_closed);
}

#[test]
fn body_available_rebind_coalesces_exact_busy_deferred_destination_owner() {
    let directory = TempDir::new().expect("temporary destination-coalescing directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for production dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8C))
        .expect("enqueue authenticated proposal");
    let proposal_effects = match runtime.step(now).expect("dispatch proposal") {
        RuntimeStep::Advanced(effects) => effects,
        RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("proposal dispatch publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let (source_tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let proposal_effect_ownership = runtime
        .take_effect_ownership(proposal_effects.len())
        .expect("FetchBody retains the proposal lifecycle owner");
    assert_eq!(proposal_effect_ownership.len(), 1);

    let body_reservation = runtime
        .reserve_body_available_with_owner(
            source_tag,
            manifest.clone(),
            &proposal_effect_ownership[0],
        )
        .expect("reserve body reconstruction under the FetchBody owner");
    runtime
        .commit_body_available(body_reservation)
        .expect("publish the owned body reconstruction completion");
    let RuntimeStep::Advanced(body_effects) =
        runtime.step(now).expect("dispatch body reconstruction")
    else {
        panic!("body reconstruction unexpectedly idled")
    };
    assert!(matches!(
        body_effects.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("body reconstruction publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let body_effect_ownership = runtime
        .take_effect_ownership(body_effects.len())
        .expect("StoreBody retains the FetchBody lifecycle owner");
    assert_eq!(body_effect_ownership.len(), 1);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &body_effect_ownership[0],
        )
        .expect("enqueue durable-store completion");
    let store_effect = body_effects[0].clone();
    let retry_store = body_effect_ownership[0].clone();
    assert_eq!(retry_store.owner(), body_effect_ownership[0].owner());
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &retry_store,
        )
        .expect("a late exact Store retry keeps the queued incumbent completion");

    let mut prepare = signed_runtime_quorum_certificate(&context, &keys, 0x8D);
    prepare.phase = wire::GlobalPhase::Prepare;
    prepare.round = manifest.round;
    prepare.proposal_round = manifest.round;
    prepare.subject = manifest.subject;
    let certified_fetch = AdapterEffect::FetchBody {
        tag: source_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(prepare.clone()),
    };
    let upgrade_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint an independently admitted certified carrier");
    let certified_fetch_owner = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(
            source_tag,
            upgrade_ordinal,
        )],
    )
    .expect("bind the independently admitted certified Fetch")
    .pop()
    .expect("one certified Fetch owns one candidate");
    let certified_store_owner = certified_fetch_owner
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("certified Fetch passes its authority to Store");
    runtime
        .enqueue_body_stored_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &certified_store_owner,
        )
        .expect("a late certified Store carrier keeps the queued incumbent completion");
    assert_eq!(runtime.queued_commands(), 1);
    let RuntimeStep::Advanced(store_effects) = runtime
        .step(now)
        .expect("dispatch durable-store completion")
    else {
        panic!("durable-store completion unexpectedly idled")
    };
    assert!(matches!(
        store_effects.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("durable-store completion publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let store_effect_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("ValidateBody retains the body pipeline lifecycle owner");
    assert_eq!(store_effect_ownership.len(), 1);
    runtime
        .enqueue_validation_succeeded_with_owner(
            source_tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
            &store_effect_ownership[0],
        )
        .expect("enqueue validation completion");
    let RuntimeStep::Advanced(validation_effects) =
        runtime.step(now).expect("dispatch validation completion")
    else {
        panic!("validation completion unexpectedly idled")
    };
    let (sign_tag, sign_preimage) = match validation_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected validation effects: {effects:?}"),
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("validation completion publishes exact scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Fifo
    );
    let sign_effect_ownership = runtime
        .take_effect_ownership(validation_effects.len())
        .expect("Prepare Sign retains the body pipeline lifecycle owner");
    assert_eq!(sign_effect_ownership.len(), 1);
    runtime
        .set_external_lifecycle_owners(vec![sign_effect_ownership[0].owner().clone()])
        .expect("publish the pending Prepare signer owner");

    let rebound = EventTag::new(
        source_tag.height(),
        source_tag.view() + 1,
        Generation::new(source_tag.generation().get() + 1),
    );
    // The body reconstructed above already owns a terminal serviced-
    // candidate record. Use another exact body to exercise the live Busy
    // lane instead of asking the adapter to resurrect that terminal.
    let rebound_manifest = runtime_manifest(&context, 0x8E);
    let (body_ordinal, body_owner) = defer_persistent_body_available_for_test(
        &mut runtime,
        source_tag,
        &rebound_manifest,
        b"body-available-retirement-owner",
    );
    let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
        manifest: rebound_manifest.clone(),
    };
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(source_tag, &evidence),
        (1, 1),
        "the current tag owns the real Busy-deferred completion"
    );
    observe_enter_view_for_test(&mut runtime, source_tag, rebound, &rebound_manifest);
    assert_eq!(
        runtime
            .driver
            .rebind_deferred_body_available(source_tag, rebound, &rebound_manifest),
        1,
        "the seam models an exact destination owner already transferred by another path"
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1),
        "the destination must be owned by the real Busy-deferred lane"
    );
    assert!(
        runtime
            .driver
            .deferred_body_available_has_persistent_producer(rebound, &rebound_manifest)
            .expect("validate the rebound durable producer"),
        "the destination must retain the sole persistent producer root"
    );
    stage_completion_for_queue_test(
        &mut runtime,
        source_tag,
        AdapterCommand::BodyAvailable {
            manifest: rebound_manifest.clone(),
        },
    );
    assert_eq!(runtime.queued_commands(), 1);

    assert!(
        runtime
            .rebind_body_available(source_tag, rebound, &rebound_manifest)
            .expect("exact destination ownership coalesces the source")
    );
    assert!(!runtime.fail_closed);
    assert_eq!(runtime.queued_commands(), 0, "the source owner was retired");
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(rebound, &evidence),
        (1, 1),
        "ordinary-source coalescing retains exactly one persistent destination owner"
    );
    assert_eq!(
        runtime
            .deferred_lifecycle_ownership
            .get(&body_ordinal)
            .map(RuntimeDeferredLifecycleOwnership::owner),
        Some(&body_owner),
        "coalescing cannot retire the wrapper of the retained Busy owner"
    );
    assert!(
        !runtime
            .rebind_body_available(source_tag, rebound, &rebound_manifest)
            .expect("an idempotent retry finds no remaining source owner")
    );
    let same_view_rebound = EventTag::new(
        rebound.height(),
        rebound.view(),
        Generation::new(rebound.generation().get() + 1),
    );
    observe_enter_view_for_test(&mut runtime, rebound, same_view_rebound, &rebound_manifest);
    assert!(
        runtime
            .rebind_body_available(rebound, same_view_rebound, &rebound_manifest)
            .expect("same-view generation supersession transfers the Busy-deferred owner")
    );
    assert_eq!(
        runtime
            .driver
            .deferred_body_pipeline_completion_ownership(same_view_rebound, &evidence),
        (1, 1),
        "same-view rebinding leaves exactly one Busy-deferred destination"
    );
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .contains_key(&body_ordinal)
    );
    assert!(
        runtime
            .retire_body_available(same_view_rebound, &rebound_manifest)
            .expect("the unique destination owner remains retireable")
    );
    assert!(
        !runtime
            .deferred_lifecycle_ownership
            .contains_key(&body_ordinal),
        "retirement cannot leave the drained Busy owner at the global minimum"
    );
    assert!(runtime.deferred_ingress_ownership.is_empty());
    let signature = Signature::new(keys[0].private_key(), &sign_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(sign_tag, signature, &sign_effect_ownership[0])
        .expect("complete the retained Prepare signer under its original owner");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the external signer after its completion is admitted");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch the retained Prepare completion"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::Broadcast(_)])
    ));

    // Exercise the opposite coalescing direction: a Busy source loses to
    // an already-installed FIFO destination. The adapter occurrence and
    // its sealed runtime wrapper must retire in the same transition.
    let retirement_directory = TempDir::new().expect("temporary Busy-source coalescing directory");
    let (mut retirement_runtime, retirement_context, _keys) =
        authenticated_network_runtime(&retirement_directory, RuntimeQueueConfig::new(8, 1, 1));
    let retirement_source = retirement_runtime.round_tag();
    let retirement_manifest = runtime_manifest(&retirement_context, 0x8F);
    retirement_runtime
        .driver
        .defer_body_pipeline_stage_for_test(
            retirement_source,
            &retirement_manifest,
            DeferredBodyPipelineStageForTest::BodyAvailable,
        )
        .expect("stage the exact Busy source completion");
    let retirement_ordinals = retirement_runtime
        .driver
        .all_deferred_admission_ordinals()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(retirement_ordinals.len(), 1);
    let retirement_ordinal = retirement_ordinals[0];
    bind_local_deferred_lifecycle_for_test(
        &mut retirement_runtime,
        retirement_ordinal,
        b"body-available-rebind-retirement-owner",
    );
    let retirement_rebound = EventTag::new(
        retirement_source.height(),
        retirement_source.view() + 1,
        Generation::new(retirement_source.generation().get() + 1),
    );
    observe_enter_view_for_test(
        &mut retirement_runtime,
        retirement_source,
        retirement_rebound,
        &retirement_manifest,
    );
    stage_completion_for_queue_test(
        &mut retirement_runtime,
        retirement_rebound,
        AdapterCommand::BodyAvailable {
            manifest: retirement_manifest.clone(),
        },
    );
    assert!(
        retirement_runtime
            .rebind_body_available(retirement_source, retirement_rebound, &retirement_manifest,)
            .expect("the existing FIFO destination coalesces the Busy source")
    );
    assert!(
        !retirement_runtime
            .deferred_lifecycle_ownership
            .contains_key(&retirement_ordinal),
        "Busy-source coalescing cannot leave its runtime wrapper alive"
    );
    assert!(
        !retirement_runtime
            .driver
            .all_deferred_admission_ordinals()
            .contains(&retirement_ordinal)
    );
    assert_eq!(retirement_runtime.queued_commands(), 1);
    assert!(
        retirement_runtime
            .retire_body_available(retirement_rebound, &retirement_manifest)
            .expect("the retained FIFO destination remains uniquely retireable")
    );
    assert_eq!(retirement_runtime.queued_commands(), 0);
}

#[test]
fn queued_body_terminal_adopts_only_authority_upgrades_and_rejects_same_authority_owners() {
    let directory = TempDir::new().expect("temporary body-terminal visibility directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for terminal-visibility dispatch");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8D))
        .expect("enqueue authenticated proposal");
    let RuntimeStep::Advanced(fetch_effects) = runtime.step(now).expect("dispatch proposal") else {
        panic!("proposal dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("proposal dispatch publishes scheduler ownership");
    let (tag, manifest) = match fetch_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    let fetch_ownership = runtime
        .take_effect_ownership(fetch_effects.len())
        .expect("take exact FetchBody ownership");
    let reservation = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &fetch_ownership[0])
        .expect("reserve exact body reconstruction");
    runtime
        .commit_body_available(reservation)
        .expect("publish body reconstruction");

    let RuntimeStep::Advanced(store_effects) =
        runtime.step(now).expect("dispatch body reconstruction")
    else {
        panic!("body reconstruction unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("body reconstruction publishes scheduler ownership");
    assert!(matches!(
        store_effects.as_slice(),
        [AdapterEffect::StoreBody { .. }]
    ));
    let store_effect = store_effects[0].clone();
    let store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("take exact StoreBody ownership");
    let commit = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Commit,
        subject: manifest.subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"terminal upgrade parent state"),
            Hash::new(b"terminal upgrade post state"),
            Hash::new(b"terminal upgrade writes"),
            1,
            Hash::new(b"terminal upgrade block"),
        ),
        signers: Vec::new(),
        aggregate_signature: Vec::new(),
    };
    let certified_fetch = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(commit),
    };
    let certified_fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 9_901)],
    )
    .expect("bind a distinct Commit-authorized Fetch owner")
    .pop()
    .expect("one Commit Fetch owner");
    let certified_store_ownership = certified_fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("Commit Fetch authorizes the exact Store stage");
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    runtime
        .enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &store_ownership[0],
        )
        .expect("queue exact durable-store terminal");
    assert_ne!(
        certified_store_ownership.candidate_semantic_identity(),
        store_ownership[0].candidate_semantic_identity(),
        "Commit authority deliberately changes the route-neutral candidate identity"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&store_effect, &certified_store_ownership)
            .expect("Commit Store observes the ordinary queued terminal")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(!runtime.fail_closed);

    let RuntimeStep::Advanced(validate_effects) =
        runtime.step(now).expect("dispatch durable-store terminal")
    else {
        panic!("durable-store terminal unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("durable-store dispatch publishes scheduler ownership");
    assert!(matches!(
        validate_effects.as_slice(),
        [AdapterEffect::ValidateBody { .. }]
    ));
    let validate_effect = validate_effects[0].clone();
    let validate_ownership = runtime
        .take_effect_ownership(validate_effects.len())
        .expect("take exact ValidateBody ownership");
    let certified_validate_ownership = certified_store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("Commit Store authorizes the exact Validate stage");
    runtime
        .enqueue_validation_succeeded_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            ValidatedBodyReceipt::for_test(durable),
            &validate_ownership[0],
        )
        .expect("queue exact validation terminal");
    assert_eq!(
        certified_validate_ownership.candidate_semantic_identity(),
        validate_ownership[0].candidate_semantic_identity(),
        "the Store terminal refinement carries Commit authority into deterministic validation"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&validate_effect, &validate_ownership[0],)
            .expect("the incumbent Commit Validate observes its queued terminal")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(!runtime.fail_closed);
    assert_ne!(
        certified_validate_ownership.owner(),
        validate_ownership[0].owner(),
        "the negative retry must carry a distinct lifecycle owner"
    );
    assert!(
        runtime
            .body_pipeline_candidate_has_terminal(&validate_effect, &certified_validate_ownership,)
            .is_err(),
        "same-authority terminal retry must reject a foreign owner"
    );
    assert!(runtime.fail_closed);
}

#[test]
fn queued_store_terminal_query_refines_prepare_to_commit_under_incumbent() {
    let directory = TempDir::new().expect("temporary terminal-refinement directory");
    let (mut runtime, context, keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
    let tag = runtime.round_tag();
    let manifest = runtime_manifest(&context, 0x8E);
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };

    let mut commit = signed_runtime_quorum_certificate(&context, &keys, 0x8F);
    commit.round = manifest.round;
    commit.proposal_round = manifest.round;
    commit.subject = manifest.subject;
    let mut prepare = commit.clone();
    prepare.phase = wire::GlobalPhase::Prepare;
    let fetch = |certificate| AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate),
    };
    let bind_store = |effect: &AdapterEffect, ordinal| {
        bind_adapter_effect_batch_ownership(
            std::slice::from_ref(effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, ordinal)],
        )
        .expect("bind one certified FetchBody carrier")
        .pop()
        .expect("one FetchBody carrier owns one candidate")
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("carry certified authority into StoreBody")
    };

    let prepare_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint Prepare-authorized StoreBody owner");
    let prepare_store = bind_store(&fetch(prepare), prepare_ordinal);
    let prepare_statement = prepare_store
        .candidate_semantic_statement()
        .expect("Prepare-authorized StoreBody carries its exact statement");
    let evidence = BodyPipelineCompletionEvidence::BodyStored {
        round: manifest.round,
        subject: manifest.subject,
        receipt: durable.clone(),
    };
    assert!(prepare_store.exactly_authorizes_body_pipeline_successor(
        &store_effect,
        tag,
        &evidence,
    ));
    stage_owned_completion_for_queue_test(
        &mut runtime,
        tag,
        AdapterCommand::BodyStored {
            round: manifest.round,
            subject: manifest.subject,
            receipt: durable,
        },
        &prepare_store,
    );
    let incumbent = runtime.ingress.commands[0]
        .lifecycle_owner()
        .expect("queued terminal retains its incumbent owner");

    let commit_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint Commit-authorized StoreBody retry owner");
    let commit_store = bind_store(&fetch(commit), commit_ordinal);
    let commit_statement = commit_store
        .candidate_semantic_statement()
        .expect("Commit-authorized StoreBody carries its exact statement");
    assert_ne!(commit_store.owner(), &incumbent);
    let planned = runtime
        .plan_body_pipeline_candidate_terminal(&store_effect, &commit_store)
        .expect("terminal query accepts the monotonic Commit refinement")
        .expect("queued StoreBody terminal produces one incumbent-owner plan");
    assert_eq!(planned.owner(), &incumbent);
    assert_eq!(
        planned.candidate_semantic_statement(),
        Some(commit_statement)
    );
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(prepare_statement),
        "planning cannot refine terminal authority before the caller's total gate"
    );
    runtime
        .commit_body_pipeline_candidate_terminal(&store_effect, &planned)
        .expect("commit the checked monotonic terminal refinement");
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.ingress.commands[0]
            .lifecycle_owner()
            .expect("authority refinement preserves the incumbent owner"),
        incumbent,
    );
    assert_eq!(
        runtime.ingress.commands[0].candidate_semantic_statement,
        Some(commit_statement),
    );
    assert!(!runtime.fail_closed);
}

#[test]
fn local_proposal_ready_is_owned_by_its_validate_predecessor() {
    let (context, _) = authenticated_runtime_context();
    let manifest = runtime_manifest(&context, 0x90);
    let tag = EventTag::new(context.height, manifest.round.view, Generation::new(4));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
        manifest: manifest.clone(),
        durable_receipt: durable.clone(),
        validated_receipt: ValidatedBodyReceipt::for_test(durable),
    };
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 9_090)],
    )
    .expect("bind exact StoreBody ownership")
    .pop()
    .expect("one StoreBody effect owns one candidate");
    let validate = store
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry the body owner into ValidateBody");

    assert!(!store.binds_body_pipeline_completion_predecessor(&evidence));
    assert!(validate.binds_body_pipeline_completion_predecessor(&evidence));
    assert!(!store.exactly_authorizes_body_pipeline_successor(&store_effect, tag, &evidence,));
    assert!(validate.exactly_authorizes_body_pipeline_successor(&validate_effect, tag, &evidence,));
}

#[test]
fn body_available_rebind_destination_conflicts_and_duplicates_fail_closed_before_mutation() {
    {
        let directory = TempDir::new().expect("temporary destination-conflict directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8D);
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        let mut conflicting = manifest.clone();
        conflicting.chunk_hashes[0] = Hash::new(b"conflicting rebound chunk");
        conflicting.chunk_root = Hash::new(b"conflicting rebound root");
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");
        runtime
            .ingress
            .enqueue_canonical_body_available(rebound, conflicting.clone())
            .expect("test seam stages conflicting destination evidence");

        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("conflicting destination evidence must fail closed"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::BodyAvailable { manifest: queued_manifest }
                if queued.tag == source_tag && queued_manifest == &manifest
        )));
        assert!(runtime.ingress.commands.iter().any(|queued| matches!(
            &queued.command,
            AdapterCommand::BodyAvailable { manifest: queued_manifest }
                if queued.tag == rebound && queued_manifest == &conflicting
        )));
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("fail-closed runtime rejects a second conflicting rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(source_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    {
        let directory = TempDir::new().expect("temporary destination-duplicate directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let source_tag = runtime.round_tag();
        let rebound = EventTag::new(
            source_tag.height(),
            source_tag.view() + 1,
            Generation::new(source_tag.generation().get() + 1),
        );
        let manifest = runtime_manifest(&context, 0x8E);
        observe_enter_view_for_test(&mut runtime, source_tag, rebound, &manifest);
        runtime
            .enqueue_body_available(source_tag, manifest.clone())
            .expect("enqueue unique source owner");
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(rebound, manifest.clone())
                .expect("test seam creates duplicate destination ownership");
        }

        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("duplicate destination ownership must fail closed"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 3);
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .filter(|queued| queued.tag == source_tag)
                .count(),
            1,
            "destination preflight must retain the source owner"
        );
        assert_eq!(
            runtime
                .ingress
                .commands
                .iter()
                .filter(|queued| queued.tag == rebound)
                .count(),
            2,
            "destination preflight must not mutate duplicate owners"
        );
        assert_eq!(
            runtime
                .rebind_body_available(source_tag, rebound, &manifest)
                .expect_err("fail-closed runtime rejects a second duplicate rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(source_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
}

#[test]
fn duplicate_body_available_rebind_and_retirement_fail_closed_before_mutation() {
    {
        let directory = TempDir::new().expect("temporary duplicate-rebind directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8E);
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(owner_tag, manifest.clone())
                .expect("test seam creates duplicate ingress ownership");
        }
        let rebound = EventTag::new(
            owner_tag.height(),
            owner_tag.view() + 1,
            Generation::new(owner_tag.generation().get() + 1),
        );
        observe_enter_view_for_test(&mut runtime, owner_tag, rebound, &manifest);

        assert_eq!(
            runtime
                .rebind_body_available(owner_tag, rebound, &manifest)
                .expect_err("duplicate ownership must prevent rebind"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.queued_commands(), 2);
        assert!(
            runtime
                .ingress
                .commands
                .iter()
                .all(|queued| queued.tag == owner_tag),
            "preflight must leave every duplicate owner at its original tag"
        );
        assert_eq!(
            runtime
                .rebind_body_available(owner_tag, rebound, &manifest)
                .expect_err("fail-closed runtime must reject a second rebind"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }

    {
        let directory = TempDir::new().expect("temporary duplicate-retirement directory");
        let (mut runtime, context, _keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 1, 1));
        let owner_tag = runtime.round_tag();
        let manifest = runtime_manifest(&context, 0x8F);
        for _ in 0..2 {
            runtime
                .ingress
                .enqueue_canonical_body_available(owner_tag, manifest.clone())
                .expect("test seam creates duplicate ingress ownership");
        }

        assert_eq!(
            runtime
                .retire_body_available(owner_tag, &manifest)
                .expect_err("duplicate ownership must prevent retirement"),
            "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
        );
        assert!(runtime.fail_closed);
        assert_eq!(
            runtime.queued_commands(),
            2,
            "preflight must not mutate duplicate serialized owners"
        );
        assert_eq!(
            runtime
                .retire_body_available(owner_tag, &manifest)
                .expect_err("fail-closed runtime must reject a second retirement"),
            "Sumeragi v2 runtime is fail-closed"
        );
        assert_eq!(
            runtime.enqueue_application_completed(owner_tag, manifest.subject),
            Err(EnqueueError::FailClosed)
        );
        assert!(matches!(
            runtime.step(Instant::now()),
            Err(RuntimeError::FailClosed)
        ));
    }
}
