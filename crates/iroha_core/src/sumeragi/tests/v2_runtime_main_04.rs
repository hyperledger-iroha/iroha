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
