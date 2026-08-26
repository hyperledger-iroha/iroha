#[test]
fn restored_pre_runtime_timeout_vote_releases_only_an_absolute_timeout_cut() {
    let directory = TempDir::new().expect("temporary restored-TV runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 2, 2),
        Some(0),
    );
    let started_at = Instant::now();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime before freezing its clock owner");
    let message = signed_runtime_timeout_vote(&context, &keys, 0, 1);
    let source = context.roster[1].validator.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), source)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("publish the pre-timeout carrier high-water mark");
    let [runtime_owned]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("fixture creates one exact durable Runtime owner");
    let receipt = runtime_owned
        .leader_wire_runtime_receipt()
        .expect("pre-crash TimeoutVote owns one exact runtime receipt")
        .clone();
    let mut restored_pre_runtime = runtime_owned;
    restored_pre_runtime.runtime_physical_cut = None;
    restored_pre_runtime.leader_wire_runtime_receipt = None;
    let pre_cut_original = restored_pre_runtime.clone();
    let non_candidate = runtime
        .timeout_vote_episode_admission_plan(None)
        .expect("unrelated ingress projects without changing TV ownership");
    assert!(matches!(
        &non_candidate,
        RuntimeTimeoutVoteEpisodeAdmissionPlan::NonCandidate
    ));
    assert_eq!(non_candidate.count_transition(), (0, 0));
    let deadline = started_at + runtime.round_timeout();
    let periodic_owner = runtime
        .mint_fresh_lifecycle_owner(
            runtime.round_tag(),
            CommandClass::Progress,
            RuntimeFreshRootKind::Retransmit,
            b"periodic-retransmit",
        )
        .expect("freeze one exact pre-timeout retransmit owner");
    runtime.retransmit_owner = Some(periodic_owner.clone());
    runtime.retransmit_owner_physical_cut = Some(runtime.ingress_physical_cut);
    let timeout_owner = runtime
        .frozen_timeout_owner_for_test(deadline)
        .expect("freeze the new process's absolute-timeout owner");
    let timeout_physical_cut = runtime
        .timeout_owner_physical_cut
        .expect("the frozen timeout owns one immutable receiver cut");
    assert!(periodic_owner.lifecycle_ordinal() < timeout_owner.lifecycle_ordinal());
    assert!(receipt.token().scheduler_ordinal() < timeout_owner.lifecycle_ordinal());
    let timeout_cut_ordinal = u64::try_from(timeout_physical_cut)
        .expect("the small test receiver cut fits its physical ordinal");
    let replay_physical_ordinal = timeout_cut_ordinal
        .max(receipt.token().admission_ordinal())
        .checked_add(1)
        .expect("the small replay ordinal has a successor");
    restored_pre_runtime.first.physical_admission_ordinal = replay_physical_ordinal;
    restored_pre_runtime.latest.physical_admission_ordinal = replay_physical_ordinal;
    assert!(restored_pre_runtime.validate_exact());
    assert_eq!(
        runtime.clock_owner_reservation_blockers_occurrence(
            receipt.token().scheduler_ordinal(),
            replay_physical_ordinal,
        ),
        Ok(RuntimeClockReservationBlockers {
            timeout: true,
            retransmit: true,
        })
    );
    assert!(
        !runtime.can_admit_timeout_vote_recovery_episode(&message, &restored_pre_runtime,),
        "the durable TimeoutIntent must execute before a restored vote can cross retained debt"
    );
    let timeout_step = runtime
        .step(deadline)
        .expect("the frozen absolute timeout runs before the restored replay");
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
    assert!(runtime.retransmit_owner.is_none());
    assert!(runtime.retransmit_owner_physical_cut.is_none());
    assert!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .is_some_and(|episode| episode.pre_frozen_retransmit.is_none())
    );
    runtime
        .freeze_due_clock_owners(deadline)
        .expect("a fresh post-timeout periodic episode remains enabled");
    let post_timeout_retransmit = runtime
        .retransmit_owner
        .as_ref()
        .expect("post-timeout retransmit owns one fresh episode");
    assert!(
        post_timeout_retransmit.lifecycle_ordinal() > timeout_owner.lifecycle_ordinal(),
        "periodic replenishment must remain outside the frozen recovery prefix"
    );
    assert_eq!(
        runtime.clock_owner_reservation_blockers_occurrence(
            receipt.token().scheduler_ordinal(),
            replay_physical_ordinal,
        ),
        Ok(RuntimeClockReservationBlockers {
            timeout: false,
            retransmit: true,
        }),
        "fresh periodic work remains a physical blocker but is above the recovery cut"
    );
    let pre_cut_candidate = runtime
        .timeout_vote_recovery_candidate_from_fair(&message.payload, &pre_cut_original)
        .expect("pre-cut TimeoutVote classification is exact")
        .expect("the original pre-cut carrier belongs to the finite episode");
    assert_eq!(
        pre_cut_candidate.owner.disposition,
        RuntimeTimeoutVoteEpisodeDisposition::PreCutDescent
    );
    assert!(
        runtime.can_admit_timeout_vote_recovery_episode(&message, &restored_pre_runtime,),
        "the exact current-view replay predates the dispatched timeout owner"
    );
    let mut restored_runtime = restored_pre_runtime;
    restored_runtime.runtime_physical_cut = u128::from(replay_physical_ordinal).checked_add(1);
    restored_runtime.leader_wire_runtime_receipt = Some(receipt.clone());
    let restored_candidate = runtime
        .timeout_vote_recovery_candidate_from_fair(&message.payload, &restored_runtime)
        .expect("restored TimeoutVote classification is exact")
        .expect("the pre-timeout owner belongs to the finite episode");
    assert_eq!(
        restored_candidate.owner.disposition,
        RuntimeTimeoutVoteEpisodeDisposition::RestoredDescent
    );
    assert!(
        restored_candidate
            .owner
            .same_lifecycle_owner_as(&pre_cut_candidate.owner),
        "the later physical replay retains the original immutable token"
    );
    assert_ne!(
        restored_candidate.owner.carrier_physical_ordinal,
        pre_cut_candidate.owner.carrier_physical_ordinal
    );
    assert_ne!(
        restored_candidate.owner.disposition,
        pre_cut_candidate.owner.disposition
    );
    let restored_owner = restored_candidate.owner.clone();
    let restored_slot = restored_candidate.slot.clone();
    let restored_plan = runtime
        .timeout_vote_episode_admission_plan(Some(restored_candidate))
        .expect("restored source-slot admission is valid");
    assert!(matches!(
        &restored_plan,
        RuntimeTimeoutVoteEpisodeAdmissionPlan::FirstAdmission { .. }
    ));
    assert_eq!(restored_plan.count_transition(), (0, 1));
    runtime
        .enqueue_network_with_ingress_ownership(message, restored_runtime)
        .expect("authenticate and enqueue the retained TimeoutVote");
    let coalesced_retry = runtime
        .timeout_vote_episode_admission_plan(Some(pre_cut_candidate))
        .expect("the same immutable owner may retry on its original carrier projection");
    assert!(matches!(
        &coalesced_retry,
        RuntimeTimeoutVoteEpisodeAdmissionPlan::CoalescedRetry { .. }
    ));
    assert_eq!(coalesced_retry.count_transition(), (1, 1));
    assert_eq!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("coalesced retry retains the timeout episode")
            .admitted_timeout_vote_owners
            .get(&restored_slot),
        Some(&restored_owner),
        "coalescing must retain rather than replace the incumbent carrier classification"
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);
    assert!(
        runtime.ingress.commands.front().is_some_and(|queued| {
            queued.class == CommandClass::Progress && !queued.command.is_certified_fence_escape()
        }),
        "TimeoutVote recovery remains ordinary Progress rather than certificate authority"
    );
    let replay_step = runtime
        .try_step_pacemaker_escape(deadline)
        .expect("ordinary Progress replay preserves the live pacemaker")
        .expect("the retained TimeoutVote receives one bounded turn");
    assert!(matches!(
        replay_step,
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    let replay_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("TimeoutVote replay publishes scheduler ownership");
    assert_eq!(
        replay_scheduler.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert!(matches!(
        replay_scheduler.candidate,
        RuntimeSelectedCandidateOwnership::Exact(ref candidate)
            if candidate.selection_seal.kind
                == RuntimeQueueSelectionKind::PacemakerProgress
    ));
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
    assert_eq!(runtime.ingress.certified_fence_escape_credit(), 0);
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    assert_eq!(
        runtime.leader_wire_runtime_receipts,
        BTreeMap::from([(receipt.token().scheduler_ordinal(), receipt)])
    );
    assert_eq!(runtime.round_tag().view(), 0);
    assert!(!runtime.fail_closed);
}
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
        leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message.clone()),
            source,
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
        runtime.emitted_timeout_recovery_owner().is_err(),
        "a changed frozen roster universe must invalidate the episode"
    );
    runtime
        .timeout_recovery_episode
        .as_mut()
        .expect("restore the exact test episode")
        .timeout_vote_owner_universe = frozen_universe;
    assert_eq!(
        runtime
            .emitted_timeout_recovery_owner()
            .expect("the restored roster universe validates")
            .map(|owner| owner.lifecycle_ordinal()),
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
            leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(message.clone()),
                source.clone(),
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
            leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(message),
                source,
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
        leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(third_message),
            third_source,
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
fn mismatched_timeout_vote_origin_is_nonfatal_and_does_not_hide_distinct_share() {
    let directory = TempDir::new().expect("temporary mismatched-TV runtime directory");
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
        .expect("arm the mismatch runtime");
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("publish the empty fair-ingress cut before timeout");
    let timeout_step = runtime
        .step(started_at + runtime.round_timeout())
        .expect("the local timeout opens its finite TV episode");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("absolute timeout unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    let timeout_effect_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("timeout Sign retains its exact lifecycle owner");
    let [timeout_effect_owner] = timeout_effect_ownership.as_slice() else {
        panic!("timeout emits one exact signing effect")
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_effect_owner.owner().clone()])
        .expect("publish the pending local timeout signer");

    let mismatched_message = signed_runtime_timeout_vote(&context, &keys, 0, 1);
    let mismatched_origin = context.roster[2].validator.clone();
    let first_valid_message = signed_runtime_timeout_vote(&context, &keys, 0, 3);
    let first_valid_origin = context.roster[3].validator.clone();
    let second_valid_message = signed_runtime_timeout_vote(&context, &keys, 0, 1);
    let second_valid_origin = context.roster[1].validator.clone();
    assert_eq!(
        second_valid_message, mismatched_message,
        "identical inner vote bytes remain independent across authenticated semantic origins"
    );
    for (message, origin) in [
        (&mismatched_message, &mismatched_origin),
        (&first_valid_message, &first_valid_origin),
        (&second_valid_message, &second_valid_origin),
    ] {
        assert!(matches!(
            leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::V2(message.clone()),
                origin.clone(),
            )),
            Ok(super::super::FairV2IngressPushDisposition::Enqueued)
        ));
    }

    for (valid_message, valid_origin) in [
        (&first_valid_message, &first_valid_origin),
        (&second_valid_message, &second_valid_origin),
    ] {
        let mut valid_inbound = leader_wire_ingress
            .try_recv_if(|inbound| {
                let BlockMessage::V2(candidate) = inbound.message() else {
                    return false;
                };
                inbound.sender() == valid_origin
                    && inbound.ingress_ownership().is_some_and(|ownership| {
                        runtime.can_admit_timeout_vote_recovery_episode(candidate, ownership)
                    })
            })
            .expect("a valid distinct share crosses the mismatched retained owner");
        let valid_ownership = valid_inbound
            .take_ingress_ownership()
            .expect("the valid share retains exact checked-dequeue ownership");
        runtime
            .enqueue_network_with_ingress_ownership(valid_message.clone(), valid_ownership)
            .expect("the valid share enters the protected Progress prefix");
    }
    assert_eq!(leader_wire_ingress.len(), 1);
    assert_eq!(runtime.queued_commands(), 2);
    assert!(
        runtime
            .ingress
            .check_capacity(CommandClass::Progress)
            .is_err(),
        "two valid shares fill the configured Progress prefix"
    );
    assert_eq!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("the timeout episode retains the valid share")
            .admitted_timeout_vote_owners
            .len(),
        2
    );

    let mut mismatched_inbound = leader_wire_ingress
        .try_recv_if(|inbound| {
            let BlockMessage::V2(candidate) = inbound.message() else {
                return false;
            };
            inbound.ingress_ownership().is_some_and(|ownership| {
                runtime.can_admit_network_message_with_ingress_ownership(candidate, ownership)
            })
        })
        .expect("the mismatched wire drains to authenticated rejection");
    let mismatched_ownership = mismatched_inbound
        .take_ingress_ownership()
        .expect("the mismatched wire retains exact terminal ownership");
    assert!(
        mismatched_ownership.leader_wire_runtime_receipt().is_some(),
        "runner terminalization receives the exact runtime receipt"
    );
    match runtime.enqueue_network_with_ingress_ownership(mismatched_message, mismatched_ownership) {
        Err(NetworkIngressError::Authentication(
            AdapterError::AuthenticatedTimeoutVoteOriginMismatch {
                signer,
                semantic_origin,
            },
        )) => {
            assert_eq!(signer, 1);
            assert_eq!(semantic_origin, mismatched_origin);
        }
        other => panic!("unexpected mismatched TimeoutVote admission: {other:?}"),
    }
    assert_eq!(runtime.queued_commands(), 2);
    assert_eq!(
        runtime
            .timeout_recovery_episode
            .as_ref()
            .expect("the timeout episode retains the valid share")
            .admitted_timeout_vote_owners
            .len(),
        2,
        "remote authentication rejection cannot consume another finite source slot"
    );
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
        restored_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message.clone()),
            source,
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
    source_substituted.direct[0].first.wire_key.origin = substituted_source.clone();
    source_substituted.direct[0].first.semantic_origin = substituted_source.clone();
    source_substituted.direct[0].first.authenticated_via = substituted_source.clone();
    source_substituted.direct[0].first.authenticated_source =
        super::super::FairV2IngressSource::Validator(substituted_source.clone());
    source_substituted.direct[0].first.semantic_owner_source =
        super::super::FairV2IngressSource::Validator(substituted_source.clone());
    source_substituted.direct[0].latest.wire_key.origin = substituted_source.clone();
    source_substituted.direct[0].latest.semantic_origin = substituted_source.clone();
    source_substituted.direct[0].latest.authenticated_via = substituted_source.clone();
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
    let authenticated_peer = super::super::authenticated_peer_for_test();
    let enqueue_authenticated =
        |ingress: &mut BoundedIngress<AdapterCommand>,
         tag: EventTag,
         class: CommandClass,
         authenticated: AuthenticatedConsensusMessage| {
            let message = authenticated.wire_envelope_for_test();
            let mut admitted = super::super::fair_v2_ingress_admit_for_test(
                super::super::InboundBlockMessage::from_authenticated_peer(
                    super::super::message::BlockMessage::V2(message.clone()),
                    authenticated_peer.clone(),
                ),
            );
            let ownership = admitted
                .take_ingress_ownership()
                .expect("real test fair ingress produces exact ownership");
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ownership)
                .expect("same-source runtime ingress projection is exact");
            ingress.enqueue_authenticated_with_ingress_ownership(
                tag,
                class,
                authenticated,
                ownership,
            )
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
        enqueue_authenticated(&mut ingress, tag(0), CommandClass::Normal, authenticated(1),)
            .expect("first wire value enters below the normal boundary"),
        tag(0)
    );
    assert_eq!(
        enqueue_authenticated(&mut ingress, tag(1), CommandClass::Normal, authenticated(2),)
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
        enqueue_authenticated(&mut ingress, tag(8), CommandClass::Normal, authenticated(1),)
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
        enqueue_authenticated(&mut ingress, tag(9), CommandClass::Normal, authenticated(3),),
        Err(EnqueueError::ReservedCapacity),
        "a non-identical envelope still obeys the normal boundary"
    );
    enqueue_authenticated(
        &mut ingress,
        tag(2),
        CommandClass::Progress,
        authenticated(3),
    )
    .expect("progress reserve remains independent");
    enqueue_authenticated(
        &mut ingress,
        tag(3),
        CommandClass::Completion,
        authenticated(4),
    )
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
        enqueue_authenticated(
            &mut ingress,
            tag(10),
            CommandClass::Normal,
            authenticated(1),
        )
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
        enqueue_authenticated(
            &mut ingress,
            tag(11),
            CommandClass::Progress,
            authenticated(5),
        ),
        Err(EnqueueError::Full),
        "wire inequality cannot inherit the duplicate's full-queue exception"
    );
}
#[test]
fn store_completion_retries_coalesce_across_ingress_and_busy_deferred_ownership() {
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
fn authenticated_remote_proposal_fetch_consumer_retag_preserves_replay_owner() {
    let directory = TempDir::new().expect("temporary remote Proposal retag directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for authenticated Proposal retag");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0xA6))
        .expect("enqueue exact authenticated Proposal");
    let RuntimeStep::Advanced(effects) = runtime.step(now).expect("dispatch Proposal") else {
        panic!("authenticated Proposal unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("Proposal publishes scheduler ownership");
    let [previous] = effects.as_slice() else {
        panic!("Proposal must emit one exact Fetch: {effects:?}")
    };
    let AdapterEffect::FetchBody {
        tag: previous_tag,
        round: previous_round,
        subject: previous_subject,
        certificate: None,
        certified_sources,
        ..
    } = previous
    else {
        panic!("authenticated Proposal must emit one ordinary Fetch")
    };
    assert!(certified_sources.is_empty());
    let ownership = runtime
        .take_effect_ownership(effects.len())
        .expect("Fetch retains exact runtime ownership")
        .pop()
        .expect("one Fetch has one exact owner");
    assert!(
        ownership
            .exact_remote_proposal_fetch_replay(previous)
            .is_some()
    );

    let rebound_tag = EventTag::new(
        previous_tag.height(),
        previous_tag.view() + 1,
        Generation::new(previous_tag.generation().get() + 1),
    );
    let mut rebound = previous.clone();
    let AdapterEffect::FetchBody { tag, .. } = &mut rebound else {
        unreachable!("fixture remains FetchBody")
    };
    *tag = rebound_tag;
    let rebound_ownership = ownership
        .rebind_fetch_consumer(previous, &rebound)
        .expect("strictly later consumer retains exact ordinary Fetch ownership");
    assert_eq!(rebound_ownership.owner(), ownership.owner());
    assert!(
        rebound_ownership
            .exact_remote_proposal_fetch_replay(&rebound)
            .is_some(),
        "the retagged Fetch must retain its authenticated Proposal envelope"
    );
    assert!(
        rebound_ownership
            .exact_remote_proposal_fetch_replay(previous)
            .is_none(),
        "the replay envelope must bind only the new consumer tag"
    );

    let mut foreign_manifest = rebound.clone();
    let AdapterEffect::FetchBody {
        manifest: Some(manifest),
        ..
    } = &mut foreign_manifest
    else {
        unreachable!("ordinary Fetch retains its Proposal manifest")
    };
    manifest.chunk_root = Hash::new(b"foreign retagged Proposal manifest");
    assert!(
        ownership
            .rebind_fetch_consumer(previous, &foreign_manifest)
            .is_err()
    );

    let mut foreign_sources = rebound.clone();
    let AdapterEffect::FetchBody {
        certified_sources, ..
    } = &mut foreign_sources
    else {
        unreachable!("fixture remains FetchBody")
    };
    certified_sources.push(PeerId::new(keys[0].public_key().clone()));
    assert!(
        ownership
            .rebind_fetch_consumer(previous, &foreign_sources)
            .is_err()
    );

    let mut certified = rebound;
    let AdapterEffect::FetchBody { certificate, .. } = &mut certified else {
        unreachable!("fixture remains FetchBody")
    };
    let mut genuine_certificate = signed_runtime_quorum_certificate(&context, &keys, 0xA5);
    genuine_certificate.round = *previous_round;
    genuine_certificate.proposal_round = *previous_round;
    genuine_certificate.subject = *previous_subject;
    *certificate = Some(genuine_certificate);
    assert!(
        ownership
            .rebind_fetch_consumer(previous, &certified)
            .is_err(),
        "certified Fetch cannot enter the ordinary Proposal retag seam"
    );

    let certified_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(&certified),
        vec![RuntimeEffectOwnership::fresh_for_test(rebound_tag, 98_001)],
    )
    .expect("bind a genuine certified Fetch owner")
    .pop()
    .expect("one certified Fetch has one exact owner");
    let mut certified_rebound = certified.clone();
    let AdapterEffect::FetchBody { tag, .. } = &mut certified_rebound else {
        unreachable!("fixture remains FetchBody")
    };
    *tag = EventTag::new(
        rebound_tag.height(),
        rebound_tag.view() + 1,
        Generation::new(rebound_tag.generation().get() + 1),
    );
    assert!(
        certified_ownership
            .rebind_fetch_consumer(&certified, &certified_rebound)
            .is_err(),
        "a genuinely certified Fetch cannot invoke the Proposal-only proof"
    );
}

#[test]
fn set_b_proposal_replay_waits_for_and_authenticates_periodic_fallback_fetch() {
    let directory = TempDir::new().expect("temporary Set-B Proposal replay directory");
    let (expected_context, _) = authenticated_runtime_context();
    let committee = crate::sumeragi::v2_core::Committee::project_indices(
        expected_context.height,
        0,
        expected_context.roster.len(),
        expected_context.leader(0),
    )
    .expect("project the deterministic view-zero committee");
    let local_validator = (0..u32::try_from(expected_context.roster.len()).expect("small roster"))
        .find(|index| {
            committee.role(*index) == Ok(crate::sumeragi::v2_core::CommitteeRole::SetBValidator)
        })
        .expect("four-validator committee has one Set-B validator");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(local_validator),
    );
    assert_eq!(context.id(), expected_context.id());
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for Set-B Proposal fallback");
    let message = signed_runtime_proposal(&context, &keys, 0xB7);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload.clone() else {
        unreachable!("signed runtime Proposal fixture carries Proposal")
    };
    runtime
        .enqueue_network(message.clone())
        .expect("enqueue exact authenticated Proposal for Set B");
    let RuntimeStep::Advanced(initial_effects) =
        runtime.step(now).expect("dispatch Set-B Proposal")
    else {
        panic!("authenticated Set-B Proposal unexpectedly idled")
    };
    assert!(
        initial_effects.is_empty(),
        "Set B must wait until periodic fallback before fetching"
    );
    runtime
        .take_last_scheduler_ownership()
        .expect("Set-B Proposal publishes scheduler ownership");
    runtime
        .take_effect_ownership(0)
        .expect("dormant Proposal emits no effect ownership yet");
    let _proposal_terminals = runtime.take_leader_wire_runtime_terminals();
    assert!(runtime.has_dormant_remote_proposal_replay());

    runtime
        .enqueue_network(message)
        .expect("an exact Proposal replay remains idempotent");
    if runtime.queued_commands() != 0 {
        let RuntimeStep::Advanced(duplicate_effects) =
            runtime.step(now).expect("dispatch exact Proposal replay")
        else {
            panic!("queued exact Proposal replay unexpectedly idled")
        };
        assert!(duplicate_effects.is_empty());
        runtime
            .take_last_scheduler_ownership()
            .expect("exact Proposal replay publishes scheduler ownership");
        runtime
            .take_effect_ownership(0)
            .expect("exact Proposal replay still emits no effect owner");
        let _duplicate_terminals = runtime.take_leader_wire_runtime_terminals();
    }
    assert!(
        runtime.has_dormant_remote_proposal_replay(),
        "an exact duplicate cannot replace or retire the incumbent replay origin"
    );

    let periodic_at = now + runtime.retransmit_interval();
    let RuntimeStep::Advanced(effects) = runtime
        .step(periodic_at)
        .expect("periodic Set-B fallback advances")
    else {
        panic!("periodic Set-B fallback unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic fallback publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer,
    );
    let fetch_position = effects
        .iter()
        .position(|effect| {
            matches!(
                effect,
                AdapterEffect::FetchBody {
                    round,
                    subject,
                    manifest: Some(manifest),
                    certified_sources,
                    certificate: None,
                    ..
                } if *round == proposal.round
                    && *subject == proposal.subject
                    && manifest == &proposal.manifest
                    && certified_sources.is_empty()
            )
        })
        .expect("periodic Set-B fallback emits the exact ordinary Proposal Fetch");
    assert_eq!(
        effects
            .iter()
            .filter(|effect| matches!(effect, AdapterEffect::FetchBody { .. }))
            .count(),
        1,
    );
    let ownership = runtime
        .take_effect_ownership(effects.len())
        .expect("periodic Set-B Fetch retains exact runtime ownership");
    assert!(
        ownership[fetch_position]
            .exact_remote_proposal_fetch_replay(&effects[fetch_position])
            .is_some(),
        "the delayed first Fetch must consume the genuine authenticated Proposal origin"
    );
    assert!(!runtime.has_dormant_remote_proposal_replay());
}

#[test]
#[allow(clippy::too_many_lines)]
fn authenticated_remote_proposal_retains_exact_fetch_store_validate_replay_origin() {
    let directory = TempDir::new().expect("temporary remote Proposal replay directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for authenticated Proposal replay");
    let proposal = signed_runtime_proposal(&context, &keys, 0xA7);
    let mut wrong_signature = proposal.clone();
    let wire::ConsensusMessageV2Payload::Proposal(wrong) = &mut wrong_signature.payload else {
        unreachable!("signed Proposal fixture has one Proposal payload")
    };
    wrong.signature[0] ^= 0xFF;
    assert!(
        runtime.enqueue_network(wrong_signature).is_err(),
        "a substituted signature cannot mint remote Proposal replay authority"
    );
    runtime
        .enqueue_network(proposal)
        .expect("enqueue exact authenticated Proposal");
    let RuntimeStep::Advanced(fetch_effects) = runtime.step(now).expect("dispatch Proposal") else {
        panic!("authenticated Proposal unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("Proposal publishes scheduler ownership");
    let [fetch_effect] = fetch_effects.as_slice() else {
        panic!("Proposal must emit one exact Fetch: {fetch_effects:?}")
    };
    let AdapterEffect::FetchBody {
        tag,
        manifest: Some(manifest),
        certificate: None,
        certified_sources,
        ..
    } = fetch_effect
    else {
        panic!("authenticated ordinary Proposal must emit manifest-bound Fetch")
    };
    assert!(certified_sources.is_empty());
    let tag = *tag;
    let manifest = manifest.clone();
    let fetch_ownership = runtime
        .take_effect_ownership(fetch_effects.len())
        .expect("Fetch retains exact runtime ownership")
        .pop()
        .expect("one Fetch has one exact owner");
    let fetch_pending = fetch_ownership
        .exact_pending_adapter_effect_binding(fetch_effect)
        .expect("Fetch owns one exact pending binding");
    let fetch_replay = fetch_ownership
        .exact_remote_proposal_fetch_replay(fetch_effect)
        .expect("authenticated Proposal attaches its replay origin");
    assert!(fetch_replay.exactly_matches_fetch_pending(fetch_effect, &fetch_pending));
    assert!(fetch_replay.exactly_matches_retry(&fetch_replay.clone(), fetch_effect,));
    let rebound_fetch_ownership = fetch_ownership
        .rebind_same_adapter_effect(fetch_effect)
        .expect("an exact Fetch retry retains its incumbent owner");
    assert!(
        rebound_fetch_ownership
            .exact_remote_proposal_fetch_replay(fetch_effect)
            .is_some(),
        "an idempotent service retry must retain authenticated Proposal replay authority"
    );
    let independent_retry_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(fetch_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 98_000)],
    )
    .expect("bind an independent semantic Fetch retry")
    .pop()
    .expect("one semantic retry has one exact owner");
    let adopted_fetch_ownership = fetch_ownership
        .adopt_incumbent_candidate_for_semantic_retry(&independent_retry_ownership, fetch_effect)
        .expect("semantic Fetch retry retains the incumbent physical owner");
    assert!(
        adopted_fetch_ownership
            .exact_remote_proposal_fetch_replay(fetch_effect)
            .is_some(),
        "semantic retry adoption must not erase authenticated Proposal replay authority"
    );
    let mut foreign_manifest_fetch = fetch_effect.clone();
    let AdapterEffect::FetchBody {
        manifest: Some(foreign_manifest),
        ..
    } = &mut foreign_manifest_fetch
    else {
        unreachable!("ordinary Fetch fixture retains one manifest")
    };
    foreign_manifest.chunk_root = Hash::new(b"foreign remote Proposal manifest root");
    assert!(
        fetch_ownership
            .exact_remote_proposal_fetch_replay(&foreign_manifest_fetch)
            .is_none()
    );
    let mut certified_fetch = fetch_effect.clone();
    let AdapterEffect::FetchBody { certificate, .. } = &mut certified_fetch else {
        unreachable!("fixture remains FetchBody")
    };
    *certificate = Some(signed_runtime_quorum_certificate(&context, &keys, 0xA8));
    assert!(
        fetch_ownership
            .exact_remote_proposal_fetch_replay(&certified_fetch)
            .is_none(),
        "certified Fetch cannot inherit ordinary Proposal replay origin"
    );
    let _proposal_terminals = runtime.take_leader_wire_runtime_terminals();
    let reservation = runtime
        .reserve_body_available_with_owner(tag, manifest.clone(), &fetch_ownership)
        .expect("reserve exact reconstructed body");
    runtime
        .commit_body_available(reservation)
        .expect("publish exact BodyAvailable successor");
    let RuntimeStep::Advanced(store_effects) = runtime.step(now).expect("dispatch BodyAvailable")
    else {
        panic!("BodyAvailable unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyAvailable publishes scheduler ownership");
    let [store_effect] = store_effects.as_slice() else {
        panic!("BodyAvailable must emit one Store: {store_effects:?}")
    };
    let store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("Store retains exact Fetch causal owner")
        .pop()
        .expect("one Store has one exact owner");
    let store_pending = store_ownership
        .exact_pending_adapter_effect_binding(store_effect)
        .expect("Store owns one exact pending binding");
    assert!(fetch_replay.exactly_projects_store(store_effect, &store_pending));
    let foreign_store_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(store_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 98_001)],
    )
    .expect("bind foreign Store root")
    .pop()
    .expect("one foreign Store owner");
    let foreign_store_pending = foreign_store_ownership
        .exact_pending_adapter_effect_binding(store_effect)
        .expect("foreign Store has one binding");
    assert!(
        fetch_replay
            .clone()
            .project_exact_store(store_effect, &foreign_store_pending)
            .is_err(),
        "matching coordinates cannot splice a foreign causal root"
    );
    let Ok(store_replay) = fetch_replay.project_exact_store(store_effect, &store_pending) else {
        panic!("exact Fetch owner projects one Store replay carrier")
    };
    assert!(store_replay.exactly_matches_store_pending(store_effect, &store_pending));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let mut foreign_manifest = manifest.clone();
    foreign_manifest.chunk_root = Hash::new(b"foreign durable remote Proposal frame");
    let foreign_durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&foreign_manifest),
    );
    assert!(
        store_replay
            .clone()
            .bind_durable_body(store_effect, &foreign_durable)
            .is_err(),
        "a substituted durable BodyFrame cannot complete Store replay evidence"
    );
    let Ok(stored_replay) = store_replay.bind_durable_body(store_effect, &durable) else {
        panic!("exact durable receipt completes Store replay evidence")
    };
    assert!(stored_replay.exactly_matches_store(store_effect, &durable));
    runtime
        .enqueue_body_stored_with_owner(
            tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
            &store_ownership,
        )
        .expect("enqueue exact durable Store completion");
    let RuntimeStep::Advanced(validate_effects) = runtime.step(now).expect("dispatch BodyStored")
    else {
        panic!("BodyStored unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyStored publishes scheduler ownership");
    let [validate_effect] = validate_effects.as_slice() else {
        panic!("BodyStored must emit one Validate: {validate_effects:?}")
    };
    let validate_ownership = runtime
        .take_effect_ownership(validate_effects.len())
        .expect("Validate retains exact Store causal owner")
        .pop()
        .expect("one Validate has one exact owner");
    let validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(validate_effect)
        .expect("Validate owns one exact pending binding");
    let foreign_validate_ownership = bind_adapter_effect_batch_ownership(
        core::slice::from_ref(validate_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag, 98_002)],
    )
    .expect("bind foreign Validate root")
    .pop()
    .expect("one foreign Validate owner");
    let foreign_validate_pending = foreign_validate_ownership
        .exact_pending_adapter_effect_binding(validate_effect)
        .expect("foreign Validate has one binding");
    assert!(
        stored_replay
            .clone()
            .project_exact_validate(
                store_effect,
                &durable,
                validate_effect,
                &foreign_validate_pending,
                None,
            )
            .is_err(),
        "matching Validate coordinates cannot splice a foreign causal root"
    );
    let Ok(validate_replay) = stored_replay.project_exact_validate(
        store_effect,
        &durable,
        validate_effect,
        &validate_pending,
        None,
    ) else {
        panic!("exact Store owner projects one Validate replay carrier")
    };
    assert!(validate_replay.exactly_matches_validate_pending(
        validate_effect,
        &durable,
        &validate_pending,
    ));
    let queued_before_drop = runtime.queued_commands();
    drop(validate_replay);
    assert_eq!(runtime.queued_commands(), queued_before_drop);
    assert!(!runtime.fail_closed);
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
        .recover_validated_body(&manifest, &validated)
        .expect("recover the exact durable execution commitment");
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
fn periodic_prepare_lock_retries_bind_store_and_validate_authority() {
    let directory = TempDir::new().expect("temporary periodic Prepare-lock directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime for Prepare-lock recovery");
    runtime
        .enqueue_network(signed_runtime_proposal(&context, &keys, 0x8E))
        .expect("enqueue authenticated proposal");
    let RuntimeStep::Advanced(proposal_effects) = runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("dispatch proposal")
    else {
        panic!("proposal dispatch unexpectedly idled")
    };
    let (proposal_tag, manifest) = match proposal_effects.as_slice() {
        [
            AdapterEffect::FetchBody {
                tag,
                manifest: Some(manifest),
                ..
            },
        ] => (*tag, manifest.clone()),
        effects => panic!("unexpected proposal effects: {effects:?}"),
    };
    runtime
        .enqueue_body_available(proposal_tag, manifest.clone())
        .expect("enqueue reconstructed proposal body");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch reconstructed body"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::StoreBody { .. }])
    ));
    let durable = DurableBodyReceipt::for_test(
        context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated = ValidatedBodyReceipt::for_test(durable.clone());
    runtime
        .enqueue_body_stored(
            proposal_tag,
            manifest.round,
            manifest.subject,
            durable.clone(),
        )
        .expect("enqueue durable proposal body");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(now)
            .expect("dispatch durable proposal body"),
        RuntimeStep::Advanced(ref effects)
            if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
    ));
    let prepare_sign_effects = runtime
        .driver_mut_for_test()
        .settle_ready_validate_succeeded_for_runtime_test(
            proposal_tag,
            manifest.round,
            manifest.subject,
            &validated,
        );
    runtime
        .retain_external_lifecycle_effect_ownership_for_test(&prepare_sign_effects)
        .expect("bind the lifecycle-owned Prepare signer");
    let prepare_sign_ownership = runtime
        .take_effect_ownership(prepare_sign_effects.len())
        .expect("Prepare signer retains exact ownership");
    let (prepare_sign_tag, prepare_signature_preimage) = match prepare_sign_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            },
        ] if vote.phase == wire::GlobalPhase::Prepare
            && vote.round == manifest.round
            && vote.subject == manifest.subject =>
        {
            (*tag, vote.signature_preimage())
        }
        effects => panic!("unexpected Prepare-sign effects: {effects:?}"),
    };
    let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(
            prepare_sign_tag,
            prepare_signature,
            &prepare_sign_ownership[0],
        )
        .expect("enqueue exact Prepare signature completion");
    let RuntimeStep::Advanced(prepare_broadcasts) = runtime
        .step(now)
        .expect("dispatch Prepare signature completion")
    else {
        panic!("Prepare signature completion unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("Prepare completion publishes scheduler ownership");
    assert!(matches!(
        prepare_broadcasts.as_slice(),
        [AdapterEffect::Broadcast(message)]
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::Vote(vote)
                    if vote.phase == wire::GlobalPhase::Prepare
                        && vote.round == manifest.round
                        && vote.subject == manifest.subject
            )
    ));
    runtime
        .take_effect_ownership(prepare_broadcasts.len())
        .expect("consume Prepare broadcast ownership");

    let prepare = wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x8F; 96],
    };
    runtime
        .ingress
        .enqueue_authenticated(
            proposal_tag,
            CommandClass::Progress,
            AuthenticatedConsensusMessage::for_test(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
            )),
        )
        .expect("enqueue the authenticated PrepareQC");
    let RuntimeStep::Advanced(lock_effects) =
        runtime.step(now).expect("install the durable Prepare lock")
    else {
        panic!("PrepareQC dispatch unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("PrepareQC publishes scheduler ownership");
    assert!(matches!(
        lock_effects.as_slice(),
        [
            AdapterEffect::Sign {
                request: SignRequest::Vote(vote),
                ..
            },
        ] if vote.phase == wire::GlobalPhase::Commit
            && vote.proposal_round == manifest.round
            && vote.subject == manifest.subject
            && vote.execution_commitment == prepare.execution_commitment
    ));
    let lock_effect_ownership = runtime
        .take_effect_ownership(lock_effects.len())
        .expect("durable lock transfers into its Commit signer");
    let _lock_terminals = runtime.take_leader_wire_runtime_terminals();
    assert_eq!(
        runtime
            .replayed_body_authority_certificate()
            .expect("query durable body authority"),
        Some(prepare.clone()),
    );
    runtime
        .set_external_lifecycle_owners(vec![lock_effect_ownership[0].owner().clone()])
        .expect("publish the pending Commit signer owner");

    runtime
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(
                signed_runtime_timeout_certificate(&context, &keys),
            ),
        ))
        .expect("enqueue the authenticated timeout certificate");
    let RuntimeStep::Advanced(enter_view_effects) = runtime
        .try_step_pacemaker_escape(now)
        .expect("the timeout certificate remains schedulable")
        .expect("the timeout certificate owns one pacemaker turn")
    else {
        panic!("timeout-certificate dispatch unexpectedly idled")
    };
    let [
        AdapterEffect::EnterView {
            tag: enter_view_tag,
            protected_lock: Some(protected_lock),
            ..
        },
        AdapterEffect::FetchBody {
            tag: fetch_tag,
            round: fetch_round,
            subject: fetch_subject,
            certificate: Some(fetch_certificate),
            ..
        },
        AdapterEffect::Sign {
            tag: commit_sign_tag,
            request: SignRequest::Vote(commit_vote),
        },
    ] = enter_view_effects.as_slice()
    else {
        panic!("unexpected protected EnterView effects: {enter_view_effects:?}")
    };
    assert_eq!(enter_view_tag.view(), proposal_tag.view() + 1);
    assert_eq!(protected_lock, &prepare);
    assert_eq!(*fetch_tag, *enter_view_tag);
    assert_eq!(*fetch_round, manifest.round);
    assert_eq!(*fetch_subject, manifest.subject);
    assert_eq!(fetch_certificate, &prepare);
    assert_eq!(commit_vote.phase, wire::GlobalPhase::Commit);
    assert_eq!(commit_vote.proposal_round, manifest.round);
    assert_eq!(commit_vote.subject, manifest.subject);
    assert_eq!(
        commit_vote.execution_commitment,
        prepare.execution_commitment
    );
    let commit_signature_preimage = commit_vote.signature_preimage();
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout certificate publishes scheduler ownership");
    let mut enter_view_ownership = runtime
        .take_effect_ownership(enter_view_effects.len())
        .expect("consume EnterView ownership");
    let commit_sign_ownership = enter_view_ownership
        .pop()
        .expect("protected EnterView retains its reissued Commit signer");
    let fetch_ownership = enter_view_ownership.swap_remove(1);
    assert_eq!(
        fetch_ownership.owner(),
        commit_sign_ownership.owner(),
        "the atomic EnterView batch retains one inherited TC lifecycle",
    );
    assert_eq!(
        fetch_ownership
            .candidate_semantic_statement()
            .expect("protected Fetch carries one statement")
            .phase,
        Some(wire::GlobalPhase::Prepare),
    );
    let _timeout_terminals = runtime.take_leader_wire_runtime_terminals();
    runtime
        .set_external_lifecycle_owners(vec![commit_sign_ownership.owner().clone()])
        .expect("replace the superseded Commit signer with its new-view retry");
    let recovery_tag = runtime.round_tag();
    assert_eq!(recovery_tag.view(), proposal_tag.view() + 1);
    runtime
        .reconcile_active_view_producer(recovery_tag, false)
        .expect("retire the non-leader view producer reservation");
    assert_eq!(
        runtime
            .replayed_body_authority_certificate()
            .expect("query post-TC durable body authority"),
        Some(prepare.clone()),
    );
    let commit_signature = Signature::new(keys[0].private_key(), &commit_signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(*commit_sign_tag, commit_signature, &commit_sign_ownership)
        .expect("enqueue the reissued Commit signature completion");
    runtime
        .set_external_lifecycle_owners(vec![fetch_ownership.owner().clone()])
        .expect("the reconstructed body remains under the shared TC owner");
    let RuntimeStep::Advanced(commit_broadcasts) = runtime
        .step_and_take_scheduler_ownership_for_test(now)
        .expect("dispatch the reissued Commit signature completion")
    else {
        panic!("reissued Commit signature completion unexpectedly idled")
    };
    assert!(matches!(
        commit_broadcasts.as_slice(),
        [AdapterEffect::Broadcast(message)]
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::Vote(vote)
                    if vote.phase == wire::GlobalPhase::Commit
                        && vote.proposal_round == manifest.round
                        && vote.subject == manifest.subject
                        && vote.execution_commitment == prepare.execution_commitment
            )
    ));
    let reservation = runtime
        .reserve_body_available_with_owner(*fetch_tag, manifest.clone(), &fetch_ownership)
        .expect("reserve locked body reconstruction under its certified Fetch owner");
    runtime
        .commit_body_available(reservation)
        .expect("publish locked body reconstruction");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the completed locked FetchBody owner");
    let RuntimeStep::Advanced(store_effects) = runtime
        .step(now)
        .expect("dispatch locked body reconstruction")
    else {
        panic!("locked body reconstruction unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("locked body reconstruction publishes scheduler ownership");
    let [incumbent_store_effect] = store_effects.as_slice() else {
        panic!("locked body reconstruction must emit one StoreBody: {store_effects:?}")
    };
    assert!(matches!(
        incumbent_store_effect,
        AdapterEffect::StoreBody { round, subject, .. }
            if *round == manifest.round && *subject == manifest.subject
    ));
    let incumbent_store_ownership = runtime
        .take_effect_ownership(store_effects.len())
        .expect("StoreBody retains the certified Fetch owner")
        .pop()
        .expect("one StoreBody has one owner");
    let store_statement = incumbent_store_ownership
        .candidate_semantic_statement()
        .expect("StoreBody carries its inherited Prepare statement");
    assert_eq!(store_statement.phase, Some(wire::GlobalPhase::Prepare));
    assert_eq!(
        store_statement.execution_commitment,
        Some(prepare.execution_commitment),
    );
    runtime
        .set_external_lifecycle_owners(vec![incumbent_store_ownership.owner().clone()])
        .expect("publish the in-flight locked StoreBody owner");

    let store_retry_at = now + runtime.retransmit_interval();
    let RuntimeStep::Advanced(store_retry_effects) = runtime
        .step(store_retry_at)
        .expect("periodic locked StoreBody retry advances")
    else {
        panic!("periodic locked StoreBody retry unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic StoreBody publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer,
    );
    let store_retry_position = store_retry_effects
        .iter()
        .position(|effect| {
            matches!(
                effect,
                AdapterEffect::StoreBody { round, subject, .. }
                    if *round == manifest.round && *subject == manifest.subject
            )
        })
        .expect("periodic recovery emits the exact StoreBody retry");
    let mut store_retry_ownership = runtime
        .take_effect_ownership(store_retry_effects.len())
        .expect("periodic StoreBody retains exact Prepare authority");
    let store_retry_ownership = store_retry_ownership.swap_remove(store_retry_position);
    assert_ne!(
        store_retry_ownership.owner(),
        incumbent_store_ownership.owner(),
        "the periodic StoreBody remains a distinct physical retry root",
    );
    assert_eq!(
        store_retry_ownership.candidate_semantic_statement(),
        Some(store_statement),
    );

    let AdapterEffect::StoreBody { tag: store_tag, .. } = incumbent_store_effect else {
        unreachable!("StoreBody shape checked above")
    };
    runtime
        .enqueue_body_stored_with_owner(
            *store_tag,
            manifest.round,
            manifest.subject,
            durable,
            &incumbent_store_ownership,
        )
        .expect("enqueue completion under the physical StoreBody incumbent");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the completed StoreBody owner");
    let RuntimeStep::Advanced(validate_effects) = runtime
        .step(store_retry_at)
        .expect("dispatch durable locked body")
    else {
        panic!("durable locked body unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("durable locked body publishes scheduler ownership");
    let [incumbent_validate_effect] = validate_effects.as_slice() else {
        panic!("durable locked body must emit one ValidateBody: {validate_effects:?}")
    };
    assert!(matches!(
        incumbent_validate_effect,
        AdapterEffect::ValidateBody { round, subject, .. }
            if *round == manifest.round && *subject == manifest.subject
    ));
    let incumbent_validate_ownership = runtime
        .take_effect_ownership(validate_effects.len())
        .expect("ValidateBody retains the physical StoreBody owner")
        .pop()
        .expect("one ValidateBody has one owner");
    assert_eq!(
        incumbent_validate_ownership.candidate_semantic_statement(),
        Some(store_statement),
    );
    runtime
        .set_external_lifecycle_owners(vec![incumbent_validate_ownership.owner().clone()])
        .expect("publish the in-flight locked ValidateBody owner");

    let validate_retry_at = store_retry_at + runtime.retransmit_interval();
    let RuntimeStep::Advanced(validate_retry_effects) = runtime
        .step(validate_retry_at)
        .expect("periodic locked ValidateBody retry advances")
    else {
        panic!("periodic locked ValidateBody retry unexpectedly idled")
    };
    assert_eq!(
        runtime
            .take_last_scheduler_ownership()
            .expect("periodic ValidateBody publishes scheduler ownership")
            .selected,
        RuntimeSelectedOwnerKind::PeriodicTimer,
    );
    let validate_retry_position = validate_retry_effects
        .iter()
        .position(|effect| {
            matches!(
                effect,
                AdapterEffect::ValidateBody { round, subject, .. }
                    if *round == manifest.round && *subject == manifest.subject
            )
        })
        .expect("periodic recovery emits the exact ValidateBody retry");
    let mut validate_retry_ownership = runtime
        .take_effect_ownership(validate_retry_effects.len())
        .expect("periodic ValidateBody retains exact Prepare authority");
    let validate_retry_ownership = validate_retry_ownership.swap_remove(validate_retry_position);
    assert_ne!(
        validate_retry_ownership.owner(),
        incumbent_validate_ownership.owner(),
        "the periodic ValidateBody remains a distinct physical retry root",
    );
    assert_eq!(
        validate_retry_ownership.candidate_semantic_statement(),
        Some(store_statement),
    );
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire the in-flight ValidateBody owner");
    assert!(!runtime.fail_closed);
}

struct PreTimeoutLockedPrepareQcRuntimeFixture {
    runtime: SerializedV2Runtime<SumeragiV2Adapter>,
    context: wire::HeightContext,
    keys: Vec<KeyPair>,
    now: Instant,
    target: PreTimeoutLockedPrepareQcTargetV1,
}

fn signed_prepare_qc_for_runtime_statement(
    keys: &[KeyPair],
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
) -> wire::QuorumCertificate {
    let signers = vec![0, 1, 2];
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small PrepareQC signer index")].private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signers,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate exact pre-timeout PrepareQC"),
    }
}

fn signed_prepare_vote_for_runtime_statement(
    keys: &[KeyPair],
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    signer: wire::ValidatorIndex,
) -> wire::ConsensusMessageV2 {
    let mut vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment,
        signer,
        signature: Vec::new(),
    };
    vote.signature = Signature::new(
        keys[usize::try_from(signer).expect("small Prepare-vote signer index")].private_key(),
        &vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote))
}

fn signed_timeout_certificate_with_highest_prepare_qc(
    keys: &[KeyPair],
    highest_prepare_qc: wire::QuorumCertificate,
) -> wire::TimeoutCertificate {
    let round = highest_prepare_qc.round;
    let signers = vec![0, 1, 2];
    let preimage = wire::TimeoutVote {
        round,
        highest_prepare_qc: Some(highest_prepare_qc.clone()),
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                keys[usize::try_from(*signer).expect("small timeout-certificate signer index")]
                    .private_key(),
                &preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    wire::TimeoutCertificate {
        round,
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: Some(highest_prepare_qc),
            signers,
            aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
                .expect("aggregate timeout certificate carrying the locked PrepareQC"),
        }],
    }
}

fn pre_timeout_locked_prepare_qc_runtime_fixture(
    directory: &TempDir,
) -> PreTimeoutLockedPrepareQcRuntimeFixture {
    const BODY_MARKER: u8 = 0xA4;

    let (expected_context, _) = authenticated_runtime_context();
    let local_validator = expected_context.leader(1);
    let local_index = usize::try_from(local_validator).expect("small local validator index");
    let (runtime_shell, context, keys) = authenticated_network_runtime_with_local_validator(
        directory,
        RuntimeQueueConfig::new(12, 2, 2),
        Some(local_validator),
    );
    assert_eq!(context.id(), expected_context.id());
    let mut adapter = runtime_shell.into_driver();

    let locked_manifest = runtime_manifest(&context, BODY_MARKER);
    let locked_durable = DurableBodyReceipt::for_test(
        context.id(),
        locked_manifest.round,
        locked_manifest.subject,
        HashOf::new(&locked_manifest),
    );
    let locked_validated = ValidatedBodyReceipt::for_test(locked_durable.clone());
    let locked_prepare = signed_prepare_qc_for_runtime_statement(
        &keys,
        locked_manifest.round,
        locked_manifest.subject,
        locked_validated.execution_commitment(),
    );
    let timeout = signed_timeout_certificate_with_highest_prepare_qc(&keys, locked_prepare.clone());
    let timeout = adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout),
        ))
        .expect("authenticate the TC carrying the older Prepare lock");
    let enter_view_effects = adapter
        .receive_authenticated(timeout)
        .expect("install the older durable Prepare lock")
        .into_effects();
    let fetch_tag = match enter_view_effects.as_slice() {
        [
            AdapterEffect::EnterView {
                tag: enter_view_tag,
                protected_lock: Some(protected_lock),
                ..
            },
            AdapterEffect::FetchBody {
                tag,
                round,
                subject,
                certificate: Some(certificate),
                ..
            },
        ] if enter_view_tag == tag
            && protected_lock == &locked_prepare
            && *round == locked_manifest.round
            && *subject == locked_manifest.subject
            && certificate == &locked_prepare =>
        {
            *tag
        }
        effects => panic!("unexpected locked EnterView effects: {effects:?}"),
    };
    assert_eq!(fetch_tag.view(), 1);
    assert_eq!(context.leader(fetch_tag.view()), local_validator);

    assert!(matches!(
        adapter
            .body_available(fetch_tag, locked_manifest.clone())
            .expect("recover the TC-protected body")
            .effects(),
        [AdapterEffect::StoreBody { tag, round, subject }]
            if *tag == fetch_tag
                && *round == locked_manifest.round
                && *subject == locked_manifest.subject
    ));
    assert!(matches!(
        adapter
            .body_stored(
                fetch_tag,
                locked_manifest.round,
                locked_manifest.subject,
                &locked_durable,
            )
            .expect("store the TC-protected body")
            .effects(),
        [AdapterEffect::ValidateBody { tag, round, subject }]
            if *tag == fetch_tag
                && *round == locked_manifest.round
                && *subject == locked_manifest.subject
    ));
    assert!(
        adapter
            .settle_ready_validate_succeeded_for_runtime_test(
                fetch_tag,
                locked_manifest.round,
                locked_manifest.subject,
                &locked_validated,
            )
            .is_empty(),
        "old-round validation must complete the Fetch without minting split-round work",
    );

    let current_round = wire::ConsensusRound {
        context_id: context.id(),
        height: fetch_tag.height(),
        view: fetch_tag.view(),
    };
    let current_manifest = encode_payload(
        &context,
        current_round,
        locked_manifest.subject,
        &[BODY_MARKER; 4],
    )
    .expect("encode the unchanged body at the current round")
    .manifest()
    .clone();
    let current_durable = DurableBodyReceipt::for_test(
        context.id(),
        current_round,
        current_manifest.subject,
        HashOf::new(&current_manifest),
    );
    let current_validated = ValidatedBodyReceipt::for_test_with_commitment(
        current_durable.clone(),
        locked_validated.execution_commitment(),
    );
    let proposal_sign = adapter
        .local_proposal_ready(
            fetch_tag,
            current_manifest,
            &current_durable,
            &current_validated,
        )
        .expect("publish the current unchanged-body reproposal")
        .into_effects();
    let proposal_sign_handoff = adapter
        .take_live_proposal_intent_wal_sign(&proposal_sign)
        .expect("the live ProposalIntent sidecar exactly matches its Sign batch")
        .expect("the fsynced local ProposalIntent retains one WAL Sign sidecar");
    let (proposal_sign_tag, proposal_sign_preimage) = match proposal_sign.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Proposal(proposal),
            },
        ] if proposal.round == current_round && proposal.subject == locked_manifest.subject => {
            (*tag, proposal.signature_preimage())
        }
        effects => panic!("unexpected current Proposal-sign effects: {effects:?}"),
    };
    let proposal_signed_effects = adapter
        .signature_completed(
            proposal_sign_tag,
            Signature::new(keys[local_index].private_key(), &proposal_sign_preimage)
                .payload()
                .to_vec(),
        )
        .expect("complete the current Proposal signature")
        .into_effects();
    drop(proposal_sign_handoff);
    assert!(
        adapter
            .take_live_proposal_intent_wal_sign(&proposal_signed_effects)
            .expect("the Proposal completion cannot leave a second live ProposalIntent sidecar")
            .is_none()
    );
    assert!(proposal_signed_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Proposal(proposal),
            ..
        }) if proposal.round == current_round && proposal.subject == locked_manifest.subject
    )));
    let (prepare_sign_tag, prepare_sign_preimage) = proposal_signed_effects
        .iter()
        .find_map(|effect| match effect {
            AdapterEffect::Sign {
                tag,
                request: SignRequest::Vote(vote),
            } if vote.phase == wire::GlobalPhase::Prepare
                && vote.round == current_round
                && vote.subject == locked_manifest.subject
                && vote.execution_commitment == locked_validated.execution_commitment() =>
            {
                Some((*tag, vote.signature_preimage()))
            }
            _ => None,
        })
        .expect("signed current Proposal emits its exact Prepare signer");
    assert_eq!(
        proposal_signed_effects.len(),
        2,
        "the signed Proposal has only its broadcast and exact Prepare signer",
    );
    assert!(matches!(
        adapter
            .signature_completed(
                prepare_sign_tag,
                Signature::new(keys[local_index].private_key(), &prepare_sign_preimage)
                    .payload()
                    .to_vec(),
            )
            .expect("complete the current Prepare signature")
            .effects(),
        [AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::Vote(vote),
            ..
        })] if vote.phase == wire::GlobalPhase::Prepare
            && vote.round == current_round
            && vote.subject == locked_manifest.subject
    ));

    let target = adapter
        .pre_timeout_locked_prepare_qc_target()
        .expect("the current validated unchanged body exposes one exact target");
    assert_eq!(target.round, current_round);
    assert_eq!(target.subject, locked_manifest.subject);
    assert_eq!(
        target.execution_commitment,
        locked_validated.execution_commitment()
    );
    assert!(adapter.ingress_ready());
    assert!(!adapter.pacemaker_escape_is_parked());
    assert!(!adapter.signature_fence_is_active());
    assert!(adapter.all_deferred_admission_ordinals().is_empty());

    let now = Instant::now();
    let (mut runtime, startup) = SerializedV2Runtime::new(
        adapter,
        Vec::new(),
        now,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(12, 2, 2),
    )
    .expect("wrap the settled adapter in the production serialized runtime");
    assert!(startup.is_empty());
    runtime
        .arm_live_clocks(now)
        .expect("arm the pre-timeout PrepareQC fixture");
    assert_eq!(runtime.round_tag(), fetch_tag);
    assert_eq!(
        runtime.driver().pre_timeout_locked_prepare_qc_target(),
        Some(target)
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert!(runtime.last_scheduler_ownership().is_none());
    assert!(!runtime.fail_closed);

    PreTimeoutLockedPrepareQcRuntimeFixture {
        runtime,
        context,
        keys,
        now,
        target,
    }
}

#[test]
fn due_timeout_dispatches_an_exact_admitted_pre_cut_locked_prepare_qc_first() {
    let directory = TempDir::new().expect("temporary pre-cut PrepareQC runtime directory");
    let PreTimeoutLockedPrepareQcRuntimeFixture {
        mut runtime,
        context,
        keys,
        now,
        target,
    } = pre_timeout_locked_prepare_qc_runtime_fixture(&directory);
    let exact_qc = signed_prepare_qc_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
    );
    let exact_message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(exact_qc));
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(exact_message.clone(), context.roster[0].validator.clone())],
        lifecycle_ordinals,
    );
    let [ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("one exact PrepareQC has one fair-ingress owner");
    let source_ordinal = ownership
        .physical_admission_ordinal()
        .expect("the exact PrepareQC owns one physical occurrence");
    let physical_cut = leader_wire_ingress.next_physical_admission_ordinal();
    assert!(u128::from(source_ordinal) < physical_cut);
    runtime
        .set_ingress_physical_cut(physical_cut)
        .expect("publish the receiver cut after the admitted PrepareQC");
    runtime
        .enqueue_network_with_ingress_ownership(exact_message.clone(), ownership)
        .expect("authenticate and admit the exact pre-cut PrepareQC");

    let deadline = now + runtime.round_timeout();
    let cut = runtime
        .freeze_pre_timeout_locked_prepare_qc_cut(deadline)
        .expect("freeze the already-due timeout owner")
        .expect("the unchanged locked body mints one pre-timeout cut");
    assert_eq!(cut.physical_cut(), physical_cut);
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &exact_message.payload));
    let Some(RuntimeStep::Advanced(effects)) = runtime
        .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
        .expect("dispatch the exact pre-cut PrepareQC")
    else {
        panic!("exact pre-cut PrepareQC did not advance")
    };
    assert!(matches!(
        effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        }] if vote.phase == wire::GlobalPhase::Commit
            && vote.round == target.round
            && vote.proposal_round == target.round
            && vote.subject == target.subject
            && vote.execution_commitment == target.execution_commitment
    ));
    assert!(!effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }
    )));
    let scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("the pre-timeout PrepareQC publishes scheduler evidence");
    assert_eq!(
        scheduler.selected,
        RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc
    );
    assert!(scheduler.timeout_due);
    assert_eq!(
        scheduler.pre_timeout_locked_prepare_qc_physical_cut,
        Some(physical_cut)
    );
    assert_eq!(scheduler.validate_exact(), Ok(()));
    let RuntimeSelectedCandidateOwnership::Exact(candidate) = &scheduler.candidate else {
        panic!("the pre-timeout PrepareQC owns one exact FIFO candidate")
    };
    assert_eq!(candidate.kind, RuntimeCommandKind::Authenticated);
    assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
    assert_eq!(
        candidate.selection_seal.kind,
        RuntimeQueueSelectionKind::PreTimeoutLockedPrepareQc
    );
    assert!(
        candidate
            .causal_origin
            .root_ingress_physical_ownership
            .is_some_and(|physical| u128::from(physical.source_ordinal) < physical_cut)
    );
    assert!(!runtime.timeout_emitted);
    runtime
        .take_effect_ownership(effects.len())
        .expect("take the pre-timeout Commit signer ownership");
    drop(runtime.take_leader_wire_runtime_terminals());
    assert!(!runtime.fail_closed);
}

#[test]
fn due_timeout_drains_two_exact_pre_cut_locked_prepare_votes_to_commit() {
    let directory = TempDir::new().expect("temporary pre-cut Prepare-vote runtime directory");
    let PreTimeoutLockedPrepareQcRuntimeFixture {
        mut runtime,
        context,
        keys,
        now,
        target,
    } = pre_timeout_locked_prepare_qc_runtime_fixture(&directory);
    let local_validator = context.leader(target.round.view);
    let remote_signers = (0..u32::try_from(context.roster.len()).expect("small fixture roster"))
        .filter(|signer| *signer != local_validator)
        .take(2)
        .collect::<Vec<_>>();
    let [first_signer, second_signer]: [wire::ValidatorIndex; 2] = remote_signers
        .try_into()
        .expect("four-validator fixture has two remote Prepare witnesses");
    let first_message = signed_prepare_vote_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
        first_signer,
    );
    let second_message = signed_prepare_vote_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
        second_signer,
    );
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[
            (
                first_message.clone(),
                context.roster[usize::try_from(first_signer).expect("small signer")]
                    .validator
                    .clone(),
            ),
            (
                second_message.clone(),
                context.roster[usize::try_from(second_signer).expect("small signer")]
                    .validator
                    .clone(),
            ),
        ],
        lifecycle_ordinals,
    );
    let physical_cut = leader_wire_ingress.next_physical_admission_ordinal();
    runtime
        .set_ingress_physical_cut(physical_cut)
        .expect("publish the receiver cut after both exact Prepare votes");
    for (message, ownership) in [first_message.clone(), second_message.clone()]
        .into_iter()
        .zip(ownerships)
    {
        assert!(
            ownership
                .physical_admission_ordinal()
                .is_some_and(|ordinal| u128::from(ordinal) < physical_cut)
        );
        runtime
            .enqueue_network_with_ingress_ownership(message, ownership)
            .expect("authenticate and admit one exact pre-cut Prepare vote");
    }

    let deadline = now + runtime.round_timeout();
    let cut = runtime
        .freeze_pre_timeout_locked_prepare_qc_cut(deadline)
        .expect("freeze the already-due timeout owner")
        .expect("the unchanged locked body mints one fixed-cut episode");
    assert_eq!(cut.physical_cut(), physical_cut);
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &first_message.payload,));
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &second_message.payload,));

    let Some(RuntimeStep::Advanced(first_effects)) = runtime
        .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
        .expect("dispatch the first exact pre-cut Prepare vote")
    else {
        panic!("first exact pre-cut Prepare vote did not advance")
    };
    assert!(
        first_effects.is_empty(),
        "the first remote witness only grows the partial Prepare pool",
    );
    let first_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("the first Prepare vote publishes scheduler evidence");
    assert_eq!(
        first_scheduler.selected,
        RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc
    );
    assert_eq!(
        first_scheduler.pre_timeout_locked_prepare_qc_physical_cut,
        Some(physical_cut)
    );
    assert_eq!(first_scheduler.validate_exact(), Ok(()));
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
    drop(runtime.take_leader_wire_runtime_terminals());
    assert!(!runtime.timeout_emitted);
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &second_message.payload,));

    let Some(RuntimeStep::Advanced(second_effects)) = runtime
        .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
        .expect("dispatch the quorum-completing pre-cut Prepare vote")
    else {
        panic!("quorum-completing pre-cut Prepare vote did not advance")
    };
    assert_eq!(second_effects.len(), 2);
    assert!(second_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::QuorumCertificate(certificate),
            ..
        }) if certificate.phase == wire::GlobalPhase::Prepare
            && certificate.round == target.round
            && certificate.proposal_round == target.round
            && certificate.subject == target.subject
            && certificate.execution_commitment == target.execution_commitment
    )));
    assert!(second_effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Sign {
            request: SignRequest::Vote(vote),
            ..
        } if vote.phase == wire::GlobalPhase::Commit
            && vote.round == target.round
            && vote.proposal_round == target.round
            && vote.subject == target.subject
            && vote.execution_commitment == target.execution_commitment
    )));
    let second_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("the quorum-completing vote publishes scheduler evidence");
    assert_eq!(
        second_scheduler.selected,
        RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc
    );
    assert_eq!(
        second_scheduler.pre_timeout_locked_prepare_qc_physical_cut,
        Some(physical_cut)
    );
    assert_eq!(second_scheduler.validate_exact(), Ok(()));
    runtime
        .take_effect_ownership(second_effects.len())
        .expect("take the exact PrepareQC broadcast and Commit signer ownership");
    drop(runtime.take_leader_wire_runtime_terminals());
    assert!(!runtime.timeout_emitted);
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
}

#[test]
fn exhausted_pre_cut_prepare_votes_do_not_grace_a_post_cut_quorum_witness() {
    let directory = TempDir::new().expect("temporary fixed-cut Prepare-vote directory");
    let PreTimeoutLockedPrepareQcRuntimeFixture {
        mut runtime,
        context,
        keys,
        now,
        target,
    } = pre_timeout_locked_prepare_qc_runtime_fixture(&directory);
    let local_validator = context.leader(target.round.view);
    let remote_signers = (0..u32::try_from(context.roster.len()).expect("small fixture roster"))
        .filter(|signer| *signer != local_validator)
        .take(2)
        .collect::<Vec<_>>();
    let [first_signer, post_cut_signer]: [wire::ValidatorIndex; 2] = remote_signers
        .try_into()
        .expect("four-validator fixture has two remote Prepare witnesses");
    let first_message = signed_prepare_vote_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
        first_signer,
    );
    let post_cut_message = signed_prepare_vote_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
        post_cut_signer,
    );
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(
            first_message.clone(),
            context.roster[usize::try_from(first_signer).expect("small signer")]
                .validator
                .clone(),
        )],
        lifecycle_ordinals,
    );
    let [first_ownership]: [FairV2IngressOwnershipEvidence; 1] = ownerships
        .try_into()
        .expect("one pre-cut Prepare vote has one fair-ingress owner");
    let physical_cut = leader_wire_ingress.next_physical_admission_ordinal();
    runtime
        .set_ingress_physical_cut(physical_cut)
        .expect("publish the receiver cut after the first Prepare vote");
    runtime
        .enqueue_network_with_ingress_ownership(first_message, first_ownership)
        .expect("authenticate and admit the sole pre-cut Prepare vote");
    let deadline = now + runtime.round_timeout();
    let cut = runtime
        .freeze_pre_timeout_locked_prepare_qc_cut(deadline)
        .expect("freeze the already-due timeout owner")
        .expect("mint the fixed-cut locked-Prepare episode");
    assert_eq!(cut.physical_cut(), physical_cut);

    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(post_cut_message.clone()),
            context.roster[usize::try_from(post_cut_signer).expect("small signer")]
                .validator
                .clone(),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut post_cut_inbound = leader_wire_ingress
        .try_recv()
        .expect("dequeue the exact post-cut Prepare vote");
    let post_cut_ownership = post_cut_inbound
        .take_ingress_ownership()
        .expect("the post-cut vote retains exact fair ownership");
    assert!(
        post_cut_ownership
            .physical_admission_ordinal()
            .is_some_and(|ordinal| u128::from(ordinal) >= physical_cut)
    );
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("refresh the live receiver high-watermark after the post-cut vote");
    runtime
        .enqueue_network_with_ingress_ownership(post_cut_message.clone(), post_cut_ownership)
        .expect("authenticate and admit the exact post-cut Prepare vote");

    let Some(RuntimeStep::Advanced(first_effects)) = runtime
        .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
        .expect("dispatch the one exact pre-cut Prepare vote")
    else {
        panic!("the exact pre-cut Prepare vote did not advance")
    };
    assert!(first_effects.is_empty());
    let first_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("the pre-cut vote publishes scheduler evidence");
    assert_eq!(
        first_scheduler.selected,
        RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc
    );
    assert_eq!(first_scheduler.validate_exact(), Ok(()));
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
    drop(runtime.take_leader_wire_runtime_terminals());
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &post_cut_message.payload,));
    assert!(
        runtime
            .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
            .expect("fixed-cut exhaustion is a successful stutter")
            .is_none()
    );
    assert!(runtime.take_last_scheduler_ownership().is_none());

    let RuntimeStep::Advanced(timeout_effects) = runtime
        .step(deadline)
        .expect("dispatch the already-owned timeout after fixed-cut exhaustion")
    else {
        panic!("ordinary due timeout unexpectedly idled")
    };
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round == target.round
    ));
    let timeout_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("the exhausted episode publishes the timeout owner");
    assert_eq!(
        timeout_scheduler.selected,
        RuntimeSelectedOwnerKind::Timeout
    );
    assert!(timeout_scheduler.timeout_due);
    assert_eq!(timeout_scheduler.validate_exact(), Ok(()));
    assert!(runtime.timeout_emitted);
    assert_eq!(runtime.queued_commands(), 1);
    runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("take the TimeoutIntent signer ownership");
    assert!(!runtime.fail_closed);
}

#[test]
fn wrong_or_post_cut_prepare_qc_gets_no_grace_before_the_due_timeout() {
    let directory = TempDir::new().expect("temporary post-cut PrepareQC runtime directory");
    let PreTimeoutLockedPrepareQcRuntimeFixture {
        mut runtime,
        context,
        keys,
        now,
        target,
    } = pre_timeout_locked_prepare_qc_runtime_fixture(&directory);
    let exact_qc = signed_prepare_qc_for_runtime_statement(
        &keys,
        target.round,
        target.subject,
        target.execution_commitment,
    );
    let exact_message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(exact_qc));
    let filler = signed_runtime_timeout_vote(&context, &keys, target.round.view, 1);
    let lifecycle_ordinals = runtime.ingress.lifecycle_ordinals.clone();
    let (_leader_wire_directory, leader_wire_ingress, filler_ownerships) =
        preowned_leader_wire_ownerships(
            &context,
            &[(filler, context.roster[1].validator.clone())],
            lifecycle_ordinals,
        );
    assert_eq!(filler_ownerships.len(), 1);
    let physical_cut = leader_wire_ingress.next_physical_admission_ordinal();
    runtime
        .set_ingress_physical_cut(physical_cut)
        .expect("publish the empty receiver cut before the exact PrepareQC");
    let deadline = now + runtime.round_timeout();
    let cut = runtime
        .freeze_pre_timeout_locked_prepare_qc_cut(deadline)
        .expect("freeze the already-due timeout owner")
        .expect("the unchanged locked body mints one pre-timeout cut");
    assert_eq!(cut.physical_cut(), physical_cut);

    let wrong_qc = signed_prepare_qc_for_runtime_statement(
        &keys,
        target.round,
        runtime_manifest(&context, 0xB6).subject,
        target.execution_commitment,
    );
    let wrong_payload = wire::ConsensusMessageV2Payload::QuorumCertificate(wrong_qc);
    assert!(!runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &wrong_payload));

    assert!(matches!(
        leader_wire_ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(exact_message.clone()),
            context.roster[0].validator.clone(),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut inbound = leader_wire_ingress
        .try_recv()
        .expect("dequeue the exact post-cut PrepareQC");
    let ownership = inbound
        .take_ingress_ownership()
        .expect("the post-cut PrepareQC retains exact fair ownership");
    let source_ordinal = ownership
        .physical_admission_ordinal()
        .expect("the post-cut PrepareQC owns one physical occurrence");
    assert!(u128::from(source_ordinal) >= cut.physical_cut());
    runtime
        .set_ingress_physical_cut(leader_wire_ingress.next_physical_admission_ordinal())
        .expect("refresh the live receiver cut after post-cut publication");
    runtime
        .enqueue_network_with_ingress_ownership(exact_message.clone(), ownership)
        .expect("authenticate and admit the exact post-cut PrepareQC");
    assert!(runtime.wire_previews_pre_timeout_locked_prepare_qc(&cut, &exact_message.payload));
    assert!(
        runtime
            .try_step_pre_timeout_locked_prepare_qc(deadline, &cut)
            .expect("post-cut absence is a successful stutter")
            .is_none()
    );
    assert!(runtime.take_last_scheduler_ownership().is_none());

    let RuntimeStep::Advanced(timeout_effects) = runtime
        .step(deadline)
        .expect("dispatch the ordinary due timeout")
    else {
        panic!("ordinary due timeout unexpectedly idled")
    };
    assert!(matches!(
        timeout_effects.as_slice(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(vote),
            ..
        }] if vote.round == target.round
    ));
    let scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("ordinary timeout publishes scheduler evidence");
    assert_eq!(scheduler.selected, RuntimeSelectedOwnerKind::Timeout);
    assert!(scheduler.timeout_due);
    assert_eq!(scheduler.validate_exact(), Ok(()));
    assert!(runtime.timeout_emitted);
    assert_eq!(runtime.queued_commands(), 1);
    runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("take the ordinary TimeoutIntent signer ownership");
    assert!(!runtime.fail_closed);
}
