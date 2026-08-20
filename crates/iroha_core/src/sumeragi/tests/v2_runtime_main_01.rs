fn bind_fake_local_deferred_target_for_test(
    runtime: &mut SerializedV2Runtime<FakeDriver>,
    semantic_identity: &[u8],
) -> u128 {
    let target_lifecycle_ordinal = runtime
        .ingress
        .mint_non_fifo_lifecycle_ordinal()
        .expect("mint one fake deferred target lifecycle");
    let target_origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
        runtime.round_tag(),
        CommandClass::Completion,
        RuntimeFreshRootKind::StartupRecovery,
        semantic_identity,
    );
    let target_owner = RuntimeLifecycleOwner::new(target_origin, target_lifecycle_ordinal)
        .expect("bind the fake deferred target lifecycle");
    let (occurrence, runtime_seal) = DeferredOccurrenceOwnershipEvidence::local_for_runtime_test(
        &runtime.driver.deferred_admission_ordinals,
        target_owner.causal_origin().lifecycle_key.clone(),
        target_owner.lifecycle_ordinal(),
        runtime.ingress_physical_cut,
    );
    let deferred_ordinal = occurrence.admission_ordinal();
    let deferred = RuntimeDeferredLifecycleOwnership::new(
        target_owner,
        deferred_ordinal,
        RuntimeDispatchIngress::LocalOrCausal,
        None,
        runtime.ingress_physical_cut,
        runtime_seal,
    )
    .expect("freeze the fake local deferred target");
    assert!(
        runtime
            .driver
            .deferred_active_ordinals
            .insert(deferred_ordinal)
    );
    assert!(
        runtime
            .driver
            .deferred_occurrence_ownership
            .insert(deferred_ordinal, occurrence)
            .is_none()
    );
    assert!(
        runtime
            .deferred_lifecycle_ownership
            .insert(deferred_ordinal, deferred)
            .is_none()
    );
    deferred_ordinal
}
fn enqueue_fake(
    runtime: &mut SerializedV2Runtime<FakeDriver>,
    tag: EventTag,
    class: CommandClass,
    command: FakeCommand,
) -> Result<(), EnqueueError> {
    runtime.enqueue(tag, class, command)
}
fn restored_fake_command(
    tag: EventTag,
    class: CommandClass,
    command: FakeCommand,
    causal_lifecycle_key: Hash,
    lifecycle_ordinal: u128,
    producer_stage: u8,
) -> TaggedCommand<FakeCommand> {
    let owner = RuntimeCandidateCausalOrigin::restore_producer_lifecycle(
        tag,
        class,
        &command,
        None,
        causal_lifecycle_key,
        lifecycle_ordinal,
    )
    .expect("validated dormant metadata reconstructs one exact owner");
    let mut tagged = TaggedCommand::with_causal_origin(
        tag,
        class,
        command,
        Instant::now(),
        owner.causal_origin().clone(),
        owner.lifecycle_ordinal(),
    )
    .expect("restored command binds its persisted ordinal");
    tagged.restored_producer_stage = Some(producer_stage);
    tagged
}
#[test]
fn successor_activation_snapshot_requires_armed_live_clocks() {
    let directory = TempDir::new().expect("temporary successor-clock directory");
    let (mut runtime, context, _keys) =
        authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
    assert!(matches!(
        runtime.successor_activation_status_snapshot(),
        Err(AdapterError::SuccessorClocksNotArmed)
    ));
    runtime
        .arm_live_clocks(Instant::now())
        .expect("arm clocks after all startup work");
    let status = runtime
        .successor_activation_status_snapshot()
        .expect("armed runtime may produce its activation snapshot");
    assert_eq!(status.height_context_id, context.id());
    assert_eq!(status.height, context.height);
    assert!(matches!(
        status.liveness.last_progress,
        Some(wire::SumeragiV2ProgressTransitionStatus {
            transition: wire::SumeragiV2ProgressTransition::SuccessorHeightActivated,
            ..
        })
    ));
}
#[test]
fn active_view_producer_cannot_fence_absolute_timeout() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xA7);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, startup) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct unarmed active-view producer runtime");
    assert!(startup.is_empty());
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve the leader producer before clocks arm");
    let reserved = runtime
        .active_view_producer
        .as_ref()
        .expect("leader producer reservation")
        .owner
        .clone();
    runtime
        .arm_live_clocks(start)
        .expect("arm clocks after producer reservation");
    assert!(
        runtime
            .local_proposal_admission_available(initial)
            .expect("armed reservation is eligible")
    );
    let ownership = runtime
        .mint_local_proposal_effect_ownership(initial, &proposal.manifest)
        .expect("local Store aliases the active producer");
    let store_effect = AdapterEffect::StoreBody {
        tag: initial,
        round: proposal.manifest.round,
        subject: proposal.manifest.subject,
    };
    let store_ownership = ownership
        .exact_store_task_ownership(&store_effect, &proposal.manifest)
        .expect("local proposal composite retains its exact Store owner");
    assert_eq!(store_ownership.owner(), &reserved);
    assert!(runtime.active_view_producer.is_some());
    let deadline = start + Duration::from_secs(10);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(deadline),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    assert!(
        runtime.active_view_producer.is_some(),
        "timeout emission must not forge proposal-fanout retirement"
    );
    runtime
        .complete_active_view_producer_after_proposal_fanout(proposal.round, &store_ownership)
        .expect("guarded fanout retires the inherited producer");
    assert!(runtime.active_view_producer.is_none());
    assert!(
        !runtime
            .local_proposal_admission_available(initial)
            .expect("consumed same-view reservation becomes retryable backpressure")
    );
    assert!(
        !runtime.fail_closed,
        "same-view scheduling churn must leave timeout recovery live"
    );
    assert!(runtime.timeout_emitted, "the view timeout remains one-shot");
}
#[test]
fn armed_proposal_admission_cannot_bypass_the_active_view_reservation() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xA9);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct unarmed runtime");
    runtime
        .reconcile_active_view_producer(initial, false)
        .expect("nonleader has no proposal reservation");
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime without a producer reservation");
    assert!(
        !runtime
            .local_proposal_admission_available(initial)
            .expect("scheduler observes an unavailable one-shot producer")
    );
    assert!(
        runtime
            .mint_local_proposal_effect_ownership(initial, &proposal.manifest)
            .is_err(),
        "the admission invariant remains fail-closed if preflight is bypassed"
    );
    assert!(runtime.fail_closed);
}
#[test]
fn replayed_proposal_fanout_consumes_the_live_producer_reservation() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xAA);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct replay runtime");
    let replay_owner = runtime
        .mint_fresh_lifecycle_owner(
            initial,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"replayed-proposal-signature",
        )
        .expect("mint exact startup recovery owner");
    let fanout_effect = AdapterEffect::StoreBody {
        tag: initial,
        round: proposal.manifest.round,
        subject: proposal.manifest.subject,
    };
    let replay_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fanout_effect),
        vec![RuntimeEffectOwnerAssignment::fresh_root(
            replay_owner,
            RuntimeFreshRootKind::StartupRecovery,
        )],
    )
    .expect("bind replayed Proposal fanout owner")
    .pop()
    .expect("one replayed Proposal fanout owner");
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve live producer after replay work was restored");
    runtime
        .arm_live_clocks(start)
        .expect("arm clocks after replay restoration");
    runtime
        .complete_active_view_producer_after_proposal_fanout(proposal.round, &replay_ownership)
        .expect("replayed original Proposal fanout consumes the live reservation");
    assert!(runtime.active_view_producer.is_none());
    assert!(!runtime.fail_closed);
}
#[test]
fn retransmitted_proposal_fanout_preserves_the_live_producer_reservation() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xAB);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct retransmit runtime");
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve live producer");
    let retransmit_owner = runtime
        .mint_fresh_lifecycle_owner(
            initial,
            CommandClass::Progress,
            RuntimeFreshRootKind::Retransmit,
            b"periodic-retransmit",
        )
        .expect("mint exact retransmit owner");
    let fanout_effect = AdapterEffect::StoreBody {
        tag: initial,
        round: proposal.manifest.round,
        subject: proposal.manifest.subject,
    };
    let retransmit_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fanout_effect),
        vec![RuntimeEffectOwnerAssignment::fresh_root(
            retransmit_owner,
            RuntimeFreshRootKind::Retransmit,
        )],
    )
    .expect("bind periodic Proposal fanout owner")
    .pop()
    .expect("one periodic Proposal fanout owner");
    runtime
        .arm_live_clocks(start)
        .expect("arm clocks after producer reservation");
    runtime
        .complete_active_view_producer_after_proposal_fanout(proposal.round, &retransmit_ownership)
        .expect("periodic Proposal fanout is not the live producer terminal");
    assert!(runtime.active_view_producer.is_some());
    assert!(!runtime.fail_closed);
}
#[test]
fn proposal_fanout_cannot_replace_active_view_producer_owner() {
    let (context, keys) = authenticated_runtime_context();
    let message = signed_runtime_proposal(&context, &keys, 0xA8);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = message.payload else {
        panic!("runtime fixture must produce a Proposal")
    };
    let initial = EventTag::new(context.height, 0, Generation::new(1));
    let start = Instant::now();
    let (mut runtime, _) = SerializedV2Runtime::with_driver(
        FakeDriver::new(initial),
        start,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
        Vec::new(),
    )
    .expect("construct active-view producer runtime");
    runtime
        .reconcile_active_view_producer(initial, true)
        .expect("reserve exact active producer");
    runtime
        .arm_live_clocks(start)
        .expect("arm after producer reservation");
    let fanout_effect = AdapterEffect::StoreBody {
        tag: initial,
        round: proposal.manifest.round,
        subject: proposal.manifest.subject,
    };
    let foreign = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fanout_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(initial, 999)],
    )
    .expect("bind foreign Proposal fanout owner")
    .pop()
    .expect("one foreign Proposal fanout owner");
    assert!(
        runtime
            .complete_active_view_producer_after_proposal_fanout(proposal.round, &foreign)
            .is_err()
    );
    assert!(runtime.fail_closed);
    assert!(runtime.active_view_producer.is_some());
}
#[test]
fn absolute_timeout_fires_once_and_messages_never_reset_it() {
    let start = Instant::now();
    let initial = tag(0);
    let mut runtime = runtime(
        FakeDriver::new(initial),
        start,
        RuntimeQueueConfig::new(8, 2, 2),
    );
    assert_eq!(runtime.remaining_completion_capacity(), 7);
    assert_eq!(runtime.round_timeout(), Duration::from_secs(10));
    assert_eq!(runtime.retransmit_interval(), Duration::from_secs(2));
    assert_eq!(runtime.watchdog_threshold(), Duration::from_secs(12));
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(1),
    )
    .expect("enqueue message");
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(1)),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(2)),
        Ok(RuntimeStep::Advanced(_))
    ));
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Normal,
        FakeCommand::record(2),
    )
    .expect("enqueue second message");
    let second_lifecycle_ordinal = runtime
        .ingress
        .commands
        .back()
        .and_then(|queued| queued.lifecycle_ordinal)
        .expect("the second message owns its immutable lifecycle ordinal");
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9))
        .expect("the admitted message precedes the fresh periodic episode");
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    assert_eq!(runtime.driver.delivered, vec![(initial, 1), (initial, 2)]);
    assert!(
        runtime
            .retransmit_owner
            .as_ref()
            .is_some_and(|owner| owner.lifecycle_ordinal() > second_lifecycle_ordinal),
        "the later runner freeze must mint a fresh periodic position after admitted work"
    );
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
        .expect("absolute timeout preempts the retained periodic episode");
    assert_eq!(runtime.driver.retransmits, vec![initial]);
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(10))
        .expect("the retained periodic episode runs immediately after timeout");
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    assert_eq!(runtime.driver.retransmits, vec![initial, initial]);
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(20))
        .expect("post-timeout scheduling succeeds");
    assert_eq!(
        runtime.driver.retransmits,
        vec![initial, initial, initial],
        "ordinary ingress never resets either clock"
    );
}
#[test]
fn absolute_timeout_preempts_serviceable_adapter_debt_then_debt_drains() {
    let start = Instant::now();
    let initial = tag(0);
    let mut driver = FakeDriver::new(initial);
    driver
        .deferred_effects
        .push_back(vec![FakeEffect::other(), FakeEffect::other()]);
    driver.deferred_effects.push_back(vec![FakeEffect::other()]);
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));
    enqueue_fake(
        &mut runtime,
        initial,
        CommandClass::Completion,
        FakeCommand::record(9),
    )
    .expect("enqueue newer runtime work");
    let due = start + Duration::from_secs(10);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    assert_eq!(runtime.driver.deferred_dispatches, 0);
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 2
    ));
    assert_eq!(runtime.driver.deferred_dispatches, 1);
    assert_eq!(runtime.queued_commands(), 1);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.deferred_dispatches, 2);
    assert_eq!(runtime.queued_commands(), 1);
    // Timeout preserves FIFO debt, so admitted work runs before the
    // still-due periodic retransmission once adapter debt is empty.
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.delivered, vec![(initial, 9)]);
    assert_eq!(runtime.queued_commands(), 0);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(due),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
    ));
    assert_eq!(runtime.driver.timeouts, vec![initial]);
    assert_eq!(runtime.driver.retransmits, vec![initial]);
}
#[test]
fn serviceable_adapter_debt_runs_without_runtime_ingress() {
    let start = Instant::now();
    let initial = tag(0);
    let mut driver = FakeDriver::new(initial);
    driver.deferred_effects.push_back(vec![FakeEffect::other()]);
    let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(8, 2, 2));
    assert_eq!(runtime.queued_commands(), 0);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
    ));
    assert_eq!(runtime.driver.deferred_dispatches, 1);
    assert!(matches!(
        runtime.step_and_take_scheduler_ownership_for_test(start),
        Ok(RuntimeStep::Idle)
    ));
}
#[test]
fn pacemaker_escape_coalesces_prequeued_distinct_origin_prepare_qc_into_live_busy_producer() {
    let directory = TempDir::new().expect("temporary producer-alias runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            signed_runtime_quorum_certificate_for_phase(
                &context,
                &keys,
                0xD7,
                wire::GlobalPhase::Prepare,
            ),
        ));
    let source_one = context.roster[1].validator.clone();
    let source_two = context.roster[2].validator.clone();
    let (_leader_wire_directory, leader_wire_ingress, ownerships) = preowned_leader_wire_ownerships(
        &context,
        &[(message.clone(), source_one), (message.clone(), source_two)],
        runtime.ingress.lifecycle_ordinals.clone(),
    );
    let [first_ownership, second_ownership]: [FairV2IngressOwnershipEvidence; 2] = ownerships
        .try_into()
        .expect("fixture creates two independently owned copies");
    let first_receipt = first_ownership
        .leader_wire_runtime_receipt()
        .expect("first route owns its runtime receipt")
        .clone();
    let second_receipt = second_ownership
        .leader_wire_runtime_receipt()
        .expect("second route owns its runtime receipt")
        .clone();
    assert_ne!(first_receipt.token(), second_receipt.token());
    let now = Instant::now();
    runtime
        .arm_live_clocks(now)
        .expect("arm runtime before opening the signing fence");
    let timeout = runtime
        .driver
        .timeout_elapsed(runtime.round_tag())
        .expect("open one local TimeoutVote signing fence");
    assert!(matches!(
        timeout.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
    runtime
        .enqueue_network_with_ingress_ownership(message.clone(), first_ownership)
        .expect("enqueue the first origin-specific PrepareQC");
    runtime
        .enqueue_network_with_ingress_ownership(message, second_ownership)
        .expect("enqueue the second origin before the first reaches Busy storage");
    assert_eq!(runtime.queued_commands(), 2);
    let first = runtime
        .try_step_pacemaker_escape(now)
        .expect("first pacemaker selection remains valid")
        .expect("first PrepareQC owns one pacemaker turn");
    assert!(matches!(first, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let first_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("first PrepareQC retains scheduler ownership");
    assert_eq!(
        first_scheduler.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    assert_eq!(
        runtime.driver().producer_continuation_counts_for_test(),
        (1, 1, 1),
        "the Busy occurrence owns one process, durable, and deferred producer alias"
    );
    assert!(runtime.take_leader_wire_runtime_terminals().is_empty());
    let second = runtime
        .try_step_pacemaker_escape(now)
        .expect("duplicate pacemaker selection must not fail closed")
        .expect("the prequeued duplicate owns one bounded retirement turn");
    assert!(matches!(second, RuntimeStep::Advanced(ref effects) if effects.is_empty()));
    let second_scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("duplicate retirement retains scheduler ownership");
    assert_eq!(
        second_scheduler.selected,
        RuntimeSelectedOwnerKind::PacemakerProgress
    );
    assert_eq!(runtime.queued_commands(), 0);
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    assert_eq!(
        runtime.driver().producer_continuation_counts_for_test(),
        (1, 1, 1),
        "an alternate route cannot mint or release the canonical Busy producer"
    );
    assert_eq!(
        runtime.leader_wire_runtime_receipts,
        BTreeMap::from([(
            first_receipt.owner().admission_ordinal(),
            first_receipt.clone(),
        )]),
        "only the canonical Busy route remains active"
    );
    let second_terminals = runtime.take_leader_wire_runtime_terminals();
    let [LeaderWireRuntimeTerminal::Volatile(retired_second)] = second_terminals.as_slice() else {
        panic!("the producer alias must retire only its outer route as volatile")
    };
    assert_eq!(retired_second, &second_receipt);
    leader_wire_ingress
        .mark_leader_wire_volatile_terminal(retired_second)
        .expect("publish the alternate route's process-local terminal");
    assert!(!runtime.fail_closed);
    let timeout_certificate =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutCertificate(
            signed_runtime_timeout_certificate(&context, &keys),
        ));
    runtime
        .enqueue_network(timeout_certificate)
        .expect("enqueue certified progress which opens the signing fence");
    let certified = runtime
        .try_step_pacemaker_escape(now)
        .expect("certified progress remains schedulable")
        .expect("the TC owns one pacemaker turn");
    let RuntimeStep::Advanced(certified_effects) = certified else {
        panic!("certified progress unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("TC retains scheduler ownership");
    runtime
        .take_effect_ownership(certified_effects.len())
        .expect("consume the TC effect ownership");
    assert!(runtime.driver().deferred_work_is_serviceable());
    let retired = runtime
        .try_step_pacemaker_escape(now)
        .expect("canonical Busy owner remains schedulable")
        .expect("canonical Busy owner receives its terminal turn");
    let RuntimeStep::Advanced(retired_effects) = retired else {
        panic!("canonical Busy owner unexpectedly idled")
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("canonical Busy owner retains scheduler ownership");
    runtime
        .take_effect_ownership(retired_effects.len())
        .expect("consume canonical Busy effects");
    let first_terminals = runtime.take_leader_wire_runtime_terminals();
    let [first_terminal] = first_terminals.as_slice() else {
        panic!("canonical Busy route emits exactly one terminal")
    };
    let retired_first = match first_terminal {
        LeaderWireRuntimeTerminal::Volatile(receipt) => {
            leader_wire_ingress
                .mark_leader_wire_volatile_terminal(receipt)
                .expect("publish canonical volatile terminal");
            receipt
        }
        LeaderWireRuntimeTerminal::Producer { runtime, terminal } => {
            leader_wire_ingress
                .mark_leader_wire_producer_terminal(runtime, *terminal)
                .expect("publish canonical producer terminal");
            runtime
        }
    };
    assert_eq!(retired_first, &first_receipt);
    assert!(runtime.leader_wire_runtime_receipts.is_empty());
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
}
#[test]
fn real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo() {
    let directory = TempDir::new().expect("temporary preowned-fence runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let first = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        signed_runtime_quorum_certificate_for_phase(
            &context,
            &keys,
            0xD8,
            wire::GlobalPhase::Prepare,
        ),
    ));
    let second = first.clone();
    let source_one = context.roster[1].validator.clone();
    let source_two = context.roster[2].validator.clone();
    let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships(
            &context,
            &[(first.clone(), source_one), (second.clone(), source_two)],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
    let [first_ownership, second_ownership]: [FairV2IngressOwnershipEvidence; 2] = ownerships
        .try_into()
        .expect("fixture creates two exact pre-timeout owners");
    let first_token = first_ownership
        .leader_wire_token()
        .expect("first aggregate owns its origin-specific token")
        .clone();
    let second_token = second_ownership
        .leader_wire_token()
        .expect("second aggregate owns its origin-specific token")
        .clone();
    let first_receipt = first_ownership
        .leader_wire_runtime_receipt()
        .expect("first aggregate owns its runtime receipt")
        .clone();
    let second_receipt = second_ownership
        .leader_wire_runtime_receipt()
        .expect("second aggregate owns its runtime receipt")
        .clone();
    assert_ne!(first_token, second_token);
    assert_ne!(first_receipt, second_receipt);
    assert_ne!(
        first_ownership
            .physical_admission_ordinal()
            .expect("first aggregate owns its physical occurrence"),
        second_ownership
            .physical_admission_ordinal()
            .expect("second aggregate owns its physical occurrence")
    );
    let start = Instant::now();
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime after preowning peer ingress");
    let deadline = start + runtime.round_timeout();
    let periodic_at = deadline
        .checked_sub(Duration::from_nanos(1))
        .expect("round deadline has a prior instant");
    let periodic_step = runtime
        .step(periodic_at)
        .expect("service the one bounded retransmit turn before Timeout");
    let periodic_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("periodic turn retains exact scheduler ownership");
    assert_eq!(
        periodic_scheduling.selected,
        RuntimeSelectedOwnerKind::PeriodicTimer
    );
    let RuntimeStep::Advanced(periodic_effects) = periodic_step else {
        panic!("pre-timeout periodic turn unexpectedly idled")
    };
    runtime
        .take_effect_ownership(periodic_effects.len())
        .expect("consume pre-timeout periodic effect ownership");
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    let timeout_step = runtime
        .step(deadline)
        .expect("absolute deadline opens TimeoutVote signing");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("absolute deadline unexpectedly idled")
    };
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("TimeoutVote Sign retains its timeout root");
    let [timeout_ownership] = timeout_ownership.as_slice() else {
        panic!("TimeoutVote Sign has one exact owner")
    };
    let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
        .expect("publish pending TimeoutVote signer owner");
    let first_physical_ordinal = first_ownership
        .physical_admission_ordinal()
        .expect("checked target owns one receiver-local occurrence");
    let first_physical_cut = first_ownership
        .runtime_physical_cut()
        .expect("checked target freezes its predecessor cut");
    runtime
        .enqueue_network_with_ingress_ownership(first, first_ownership)
        .expect("admit first pre-timeout peer owner after signing begins");
    runtime
        .enqueue_network_with_ingress_ownership(second, second_ownership)
        .expect("admit the distinct-origin duplicate before either aggregate dispatches");
    assert_eq!(runtime.queued_commands(), 2);
    assert_eq!(
        runtime
            .active_leader_wire_runtime_ordinals()
            .expect("both durable aggregate owners remain active"),
        BTreeSet::from([
            first_token.scheduler_ordinal(),
            second_token.scheduler_ordinal(),
        ])
    );
    assert_eq!(runtime.leader_wire_runtime_receipts.len(), 2);
    runtime
        .set_ingress_physical_cut(
            first_physical_cut
                .checked_add(2)
                .expect("small test cut can advance"),
        )
        .expect("later receiver activity advances only the global high-watermark");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("move first peer owner into Busy-deferred state"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    assert!(!runtime.driver().deferred_work_is_serviceable());
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(runtime.deferred_ingress_ownership.len(), 1);
    assert_eq!(runtime.deferred_lifecycle_ownership.len(), 1);
    let (&deferred_ordinal, deferred_target) = runtime
        .deferred_lifecycle_ownership
        .iter()
        .next()
        .expect("Busy target retains exact lifecycle ownership");
    let deferred_target = deferred_target.clone();
    assert_eq!(
        deferred_target.source_physical_ordinal,
        Some(first_physical_ordinal)
    );
    assert_eq!(
        deferred_target.physical_cut, first_physical_cut,
        "a later global receiver high-watermark cannot refresh the target cut"
    );
    assert_eq!(
        runtime.deferred_ingress_ownership[&deferred_ordinal].leader_wire_token(),
        Ok(Some(&first_token)),
        "the Busy occurrence owns only the selected origin-specific lifecycle"
    );
    assert!(runtime.take_leader_wire_runtime_terminals().is_empty());
    let queue_before_fenced_idle = runtime.ingress.ownership_snapshot();
    assert!(matches!(
        runtime
            .step(deadline)
            .expect("the later duplicate cannot cross the active signing fence"),
        RuntimeStep::Idle
    ));
    let fenced_idle = runtime
        .take_last_scheduler_ownership()
        .expect("fenced idle retains exact scheduler evidence");
    assert_eq!(fenced_idle.selected, RuntimeSelectedOwnerKind::Idle);
    assert_eq!(fenced_idle.validate_exact(), Ok(()));
    assert_eq!(
        fenced_idle.queue_before,
        queue_before_fenced_idle.projection
    );
    assert_eq!(fenced_idle.queue_after, queue_before_fenced_idle.projection);
    assert!(!fenced_idle.fifo_ready);
    assert!(!fenced_idle.completion_ready);
    assert!(!fenced_idle.progress_ready);
    assert!(!fenced_idle.normal_ready);
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.deferred_lifecycle_ownership[&deferred_ordinal], deferred_target,
        "an idle fenced turn cannot replace the Busy ordinal, seal, or frozen cut"
    );
    assert_eq!(runtime.leader_wire_runtime_receipts.len(), 2);
    assert!(runtime.take_leader_wire_runtime_terminals().is_empty());
    let signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(sign_tag, signature, timeout_ownership)
        .expect("enqueue exact owned TimeoutVote completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire pending signer after completion enqueue");
    let completion_step = runtime
        .step(deadline)
        .expect("exact completion crosses preowned fenced FIFO debt");
    let scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("dependency bypass retains scheduler evidence");
    assert_eq!(
        scheduling.selected,
        RuntimeSelectedOwnerKind::FenceCompletion
    );
    assert!(scheduling.fence_completion_bypass);
    assert!(scheduling.validate_exact().is_ok());
    assert!(
        scheduling
            .fence_predecessor_ingress_ownership
            .as_ref()
            .is_some_and(RuntimeIngressOwnershipEvidence::validate_frozen_physical),
        "an authenticated fence target retains its checked ingress carrier"
    );
    assert_eq!(
        scheduling
            .fence_predecessor_ingress_ownership
            .as_ref()
            .expect("fence target retains ingress ownership")
            .leader_wire_token(),
        Ok(Some(&first_token)),
        "the dependency bypass names the Busy aggregate, never its later duplicate"
    );
    let mut weakened_fence = scheduling.clone();
    weakened_fence
        .fence_predecessor_ownership
        .as_mut()
        .expect("fence evidence carries its exact deferred target")
        .physical_cut = first_physical_cut
        .checked_add(1)
        .expect("small test cut can be mutated");
    weakened_fence.projection_hash = runtime_scheduler_projection_hash(&weakened_fence);
    assert_eq!(
        weakened_fence.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "rehashing cannot hide a fence-target physical-cut mutation"
    );
    let mut replenished_fence_debt = scheduling.clone();
    replenished_fence_debt.queue_after.max_service_debt = replenished_fence_debt
        .queue_before
        .max_service_debt
        .saturating_add(1);
    replenished_fence_debt.projection_hash =
        runtime_scheduler_projection_hash(&replenished_fence_debt);
    assert_eq!(
        replenished_fence_debt.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "the dependency-only fence branch cannot replenish scheduler debt"
    );
    let mut coherently_weakened_fence = scheduling.clone();
    let mutated_cut = first_physical_cut
        .checked_add(1)
        .expect("small test cut can be mutated");
    let predecessor = coherently_weakened_fence
        .fence_predecessor_ownership
        .as_mut()
        .expect("fence evidence carries its exact deferred target");
    predecessor.physical_cut = mutated_cut;
    predecessor
        .owner
        .causal_origin
        .root_ingress_physical_ownership
        .as_mut()
        .expect("network-rooted target carries its physical pair")
        .physical_cut = mutated_cut;
    predecessor.owner.causal_origin.projection_hash =
        runtime_candidate_causal_origin_projection_hash(&predecessor.owner.causal_origin);
    predecessor.owner.projection_hash = runtime_lifecycle_owner_projection_hash(&predecessor.owner);
    coherently_weakened_fence.projection_hash =
        runtime_scheduler_projection_hash(&coherently_weakened_fence);
    assert_eq!(
        coherently_weakened_fence.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "the retained fair-ingress carrier rejects a coherently rehashed wrapper/root cut mutation"
    );
    let mut deleted_fence_ingress = scheduling.clone();
    deleted_fence_ingress.fence_predecessor_ingress_ownership = None;
    deleted_fence_ingress.projection_hash =
        runtime_scheduler_projection_hash(&deleted_fence_ingress);
    assert_eq!(
        deleted_fence_ingress.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "direct-authenticated provenance rejects deletion of the rehashed fence carrier"
    );
    let mut reclassified_fence = scheduling.clone();
    reclassified_fence.fence_predecessor_ingress_ownership = None;
    reclassified_fence
        .fence_predecessor_ownership
        .as_mut()
        .expect("fence evidence carries its exact deferred target")
        .current_ingress = RuntimeDispatchIngress::LocalOrCausal;
    reclassified_fence.projection_hash = runtime_scheduler_projection_hash(&reclassified_fence);
    assert_eq!(
        reclassified_fence.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "the adapter-issued occurrence capability rejects a coherent provenance flip"
    );
    let RuntimeStep::Advanced(effects) = completion_step else {
        panic!("exact TimeoutVote completion unexpectedly idled")
    };
    assert!(effects.iter().any(|effect| matches!(
        effect,
        AdapterEffect::Broadcast(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                    if vote.round.height == context.height && vote.round.view == 0
            )
    )));
    runtime
        .take_effect_ownership(effects.len())
        .expect("consume TimeoutVote broadcast ownership");
    let deferred_step = runtime
        .step(deadline)
        .expect("the physically frozen Busy target owns the next turn");
    let deferred_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("deferred turn retains scheduler evidence");
    let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
        &deferred_scheduling.candidate
    else {
        panic!("expected exact deferred scheduler ownership")
    };
    assert_eq!(candidate.service.admission_ordinal, deferred_ordinal);
    assert_eq!(candidate.lifecycle_ownership, deferred_target);
    assert_eq!(
        candidate
            .ingress_ownership
            .as_ref()
            .expect("deferred aggregate retains its authenticated carrier")
            .leader_wire_token(),
        Ok(Some(&first_token))
    );
    assert_eq!(
        candidate.lifecycle_ownership.source_physical_ordinal,
        Some(first_physical_ordinal)
    );
    assert_eq!(
        candidate.lifecycle_ownership.physical_cut,
        first_physical_cut
    );
    assert_eq!(deferred_scheduling.validate_exact(), Ok(()));
    let mut weakened_deferred = deferred_scheduling.clone();
    let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
        &mut weakened_deferred.candidate
    else {
        unreachable!("cloned deferred evidence retains its variant")
    };
    candidate.lifecycle_ownership.physical_cut = first_physical_cut
        .checked_add(1)
        .expect("small test cut can be mutated");
    weakened_deferred.projection_hash = runtime_scheduler_projection_hash(&weakened_deferred);
    assert_eq!(
        weakened_deferred.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "rehashing cannot hide a deferred-target physical-cut mutation"
    );
    let mut ordinal_mutation = deferred_scheduling.clone();
    let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
        &mut ordinal_mutation.candidate
    else {
        unreachable!("cloned deferred evidence retains its variant")
    };
    candidate.lifecycle_ownership.deferred_admission_ordinal = candidate
        .lifecycle_ownership
        .deferred_admission_ordinal
        .checked_add(1)
        .expect("small adapter ordinal has a successor");
    ordinal_mutation.projection_hash = runtime_scheduler_projection_hash(&ordinal_mutation);
    assert_eq!(
        ordinal_mutation.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "a rehashed wrapper cannot detach from the selected adapter ordinal"
    );
    let mut nonminimum_rebase = deferred_scheduling.clone();
    let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
        &mut nonminimum_rebase.candidate
    else {
        unreachable!("cloned deferred evidence retains its variant")
    };
    let invalid_lower_rank = candidate
        .lifecycle_ownership
        .owner
        .lifecycle_ordinal
        .checked_sub(1)
        .expect("aggregate fixture has a lower nonminimum rank");
    candidate.lifecycle_ownership.owner.lifecycle_ordinal = invalid_lower_rank;
    candidate
        .lifecycle_ownership
        .owner
        .causal_origin
        .root_lifecycle_ordinal = Some(invalid_lower_rank);
    candidate
        .lifecycle_ownership
        .owner
        .causal_origin
        .projection_hash = runtime_candidate_causal_origin_projection_hash(
        &candidate.lifecycle_ownership.owner.causal_origin,
    );
    candidate.lifecycle_ownership.owner.projection_hash =
        runtime_lifecycle_owner_projection_hash(&candidate.lifecycle_ownership.owner);
    nonminimum_rebase.projection_hash = runtime_scheduler_projection_hash(&nonminimum_rebase);
    assert_eq!(
        nonminimum_rebase.validate_exact(),
        Err(RuntimeSchedulerEvidenceError::InvalidProjection),
        "aggregate rebasing must equal the retained ingress minimum, not any lower rank"
    );
    let RuntimeStep::Advanced(deferred_effects) = deferred_step else {
        panic!("deferred target unexpectedly idled")
    };
    runtime
        .take_effect_ownership(deferred_effects.len())
        .expect("consume deferred target effect ownership");
    let first_terminals = runtime.take_leader_wire_runtime_terminals();
    let [first_terminal] = first_terminals.as_slice() else {
        panic!("servicing the first aggregate emits exactly its one terminal")
    };
    let first_terminal_receipt = match first_terminal {
        LeaderWireRuntimeTerminal::Volatile(receipt)
        | LeaderWireRuntimeTerminal::Producer {
            runtime: receipt, ..
        } => receipt,
    };
    assert_eq!(first_terminal_receipt, &first_receipt);
    assert_eq!(
        runtime.leader_wire_runtime_receipts,
        BTreeMap::from([(second_token.scheduler_ordinal(), second_receipt.clone(),)]),
        "the first terminal cannot consume the later origin-specific receipt"
    );
    let second_step = runtime
        .step(deadline)
        .expect("the later duplicate runs only after the Busy owner terminalizes");
    let second_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("the later duplicate retains its independent FIFO owner");
    assert_eq!(second_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
    let RuntimeSelectedCandidateOwnership::Exact(second_candidate) = &second_scheduling.candidate
    else {
        panic!("the later duplicate must remain an independent FIFO lifecycle")
    };
    assert_eq!(
        second_candidate.lifecycle_ordinal,
        second_token.scheduler_ordinal()
    );
    let RuntimeStep::Advanced(second_effects) = second_step else {
        panic!("the later aggregate unexpectedly idled after its predecessor terminalized")
    };
    runtime
        .take_effect_ownership(second_effects.len())
        .expect("consume later aggregate effect ownership");
    let second_terminals = runtime.take_leader_wire_runtime_terminals();
    let [second_terminal] = second_terminals.as_slice() else {
        panic!("the later aggregate emits exactly its own terminal")
    };
    let second_terminal_receipt = match second_terminal {
        LeaderWireRuntimeTerminal::Volatile(receipt)
        | LeaderWireRuntimeTerminal::Producer {
            runtime: receipt, ..
        } => receipt,
    };
    assert_eq!(second_terminal_receipt, &second_receipt);
    assert!(runtime.leader_wire_runtime_receipts.is_empty());
    assert!(runtime.deferred_ingress_ownership.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
}
#[test]
fn real_adapter_fence_services_unblocked_predecessor_before_completion() {
    let directory = TempDir::new().expect("temporary mixed-fence runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let target = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        signed_runtime_quorum_certificate_for_phase(
            &context,
            &keys,
            0xD8,
            wire::GlobalPhase::Prepare,
        ),
    ));
    let blocked = signed_runtime_timeout_vote(&context, &keys, 0, 2);
    let safe = signed_runtime_timeout_vote(&context, &keys, 2, 3);
    let target_source = context.roster[1].validator.clone();
    let blocked_source = context.roster[2].validator.clone();
    let safe_source = context.roster[3].validator.clone();
    let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships_at_shared_cut(
            &context,
            &[
                (target.clone(), target_source),
                (blocked.clone(), blocked_source),
                (safe.clone(), safe_source),
            ],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
    let [target_ownership, blocked_ownership, safe_ownership]: [FairV2IngressOwnershipEvidence; 3] =
        ownerships
            .try_into()
            .expect("fixture creates three exact owners at one checked-dequeue cut");
    let target_token = target_ownership
        .leader_wire_token()
        .expect("target owns a durable runtime token")
        .clone();
    let blocked_token = blocked_ownership
        .leader_wire_token()
        .expect("blocked peer input owns a durable runtime token")
        .clone();
    let safe_token = safe_ownership
        .leader_wire_token()
        .expect("safe predecessor owns a durable runtime token")
        .clone();
    let target_receipt = target_ownership
        .leader_wire_runtime_receipt()
        .expect("target owns a durable runtime receipt")
        .clone();
    let blocked_receipt = blocked_ownership
        .leader_wire_runtime_receipt()
        .expect("blocked input owns a durable runtime receipt")
        .clone();
    let safe_receipt = safe_ownership
        .leader_wire_runtime_receipt()
        .expect("safe predecessor owns a durable runtime receipt")
        .clone();
    let target_cut = target_ownership
        .runtime_physical_cut()
        .expect("shared dequeue freezes one physical cut");
    for ownership in [&target_ownership, &blocked_ownership, &safe_ownership] {
        assert!(
            u128::from(
                ownership
                    .physical_admission_ordinal()
                    .expect("each fixture input owns a physical occurrence")
            ) < target_cut,
            "every mixed-fence owner must physically precede the target cut"
        );
    }
    let start = Instant::now();
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime after preowning mixed peer ingress");
    runtime
        .step_and_take_scheduler_ownership_for_test(start + Duration::from_secs(9))
        .expect("service the pre-fence retransmission episode");
    let deadline = start + runtime.round_timeout();
    let timeout_step = runtime
        .step(deadline)
        .expect("absolute deadline opens TimeoutVote signing");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("absolute deadline unexpectedly idled")
    };
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("TimeoutVote Sign retains its timeout root");
    let [timeout_ownership] = timeout_ownership.as_slice() else {
        panic!("TimeoutVote Sign has one exact owner")
    };
    let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
        .expect("publish pending TimeoutVote signer owner");
    runtime
        .enqueue_network_with_ingress_ownership(target, target_ownership)
        .expect("admit deferred target from the shared cut");
    runtime
        .enqueue_network_with_ingress_ownership(blocked, blocked_ownership)
        .expect("admit independently blocked peer input");
    runtime
        .enqueue_network_with_ingress_ownership(safe, safe_ownership)
        .expect("admit far-future TimeoutVote which terminates before the reducer");
    assert_eq!(runtime.queued_commands(), 3);
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("move target into Busy-deferred ownership"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    assert!(!runtime.driver().deferred_work_is_serviceable());
    assert_eq!(runtime.queued_commands(), 2);
    let queue_before_predecessor = runtime.ingress.ownership_snapshot();
    let (&deferred_ordinal, deferred_target) = runtime
        .deferred_lifecycle_ownership
        .iter()
        .next()
        .expect("target retains one exact Busy occurrence");
    let deferred_target = deferred_target.clone();
    let predecessor_step = runtime
        .step(deadline)
        .expect("oldest safe pre-cut owner runs before the fence completion");
    let predecessor_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("fence predecessor retains scheduler evidence");
    assert_eq!(
        predecessor_scheduling.selected,
        RuntimeSelectedOwnerKind::FencePredecessor
    );
    assert!(!predecessor_scheduling.fence_completion_bypass);
    assert_eq!(predecessor_scheduling.validate_exact(), Ok(()));
    let RuntimeSelectedCandidateOwnership::Exact(predecessor_candidate) =
        &predecessor_scheduling.candidate
    else {
        panic!("safe predecessor retains exact FIFO ownership")
    };
    assert_eq!(
        predecessor_candidate.lifecycle_ordinal,
        safe_token.scheduler_ordinal()
    );
    assert_eq!(
        predecessor_scheduling.queue_before.service_cursor,
        predecessor_scheduling.queue_after.service_cursor,
        "dependency predecessor cannot advance ordinary class rotation"
    );
    assert_eq!(
        predecessor_scheduling.queue_before.max_service_debt,
        predecessor_scheduling.queue_after.max_service_debt,
        "dependency predecessor cannot replenish ordinary service debt"
    );
    assert_eq!(
        predecessor_scheduling.queue_before,
        queue_before_predecessor.projection
    );
    let RuntimeStep::Advanced(predecessor_effects) = predecessor_step else {
        panic!("safe predecessor unexpectedly idled")
    };
    assert!(predecessor_effects.is_empty());
    assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
    assert_eq!(runtime.queued_commands(), 1);
    assert_eq!(
        runtime.deferred_lifecycle_ownership[&deferred_ordinal],
        deferred_target
    );
    assert_eq!(
        runtime.leader_wire_runtime_receipts,
        BTreeMap::from([
            (target_token.scheduler_ordinal(), target_receipt.clone()),
            (blocked_token.scheduler_ordinal(), blocked_receipt.clone()),
        ])
    );
    let safe_terminals = runtime.take_leader_wire_runtime_terminals();
    let [safe_terminal] = safe_terminals.as_slice() else {
        panic!("safe predecessor emits exactly its own terminal")
    };
    let safe_terminal_receipt = match safe_terminal {
        LeaderWireRuntimeTerminal::Volatile(receipt)
        | LeaderWireRuntimeTerminal::Producer {
            runtime: receipt, ..
        } => receipt,
    };
    assert_eq!(safe_terminal_receipt, &safe_receipt);
    let signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(sign_tag, signature, timeout_ownership)
        .expect("enqueue exact owned TimeoutVote completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire pending signer after completion enqueue");
    let completion_step = runtime
        .step(deadline)
        .expect("completion follows the retired safe predecessor");
    let completion_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("completion retains dependency evidence");
    assert_eq!(
        completion_scheduling.selected,
        RuntimeSelectedOwnerKind::FenceCompletion
    );
    assert_eq!(completion_scheduling.validate_exact(), Ok(()));
    let RuntimeStep::Advanced(completion_effects) = completion_step else {
        panic!("exact completion unexpectedly idled")
    };
    runtime
        .take_effect_ownership(completion_effects.len())
        .expect("consume completion effects");
    let deferred_step = runtime
        .step(deadline)
        .expect("opened Busy target drains after its completion");
    let deferred_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("deferred target retains scheduler ownership");
    assert_eq!(
        deferred_scheduling.selected,
        RuntimeSelectedOwnerKind::Deferred
    );
    let RuntimeStep::Advanced(deferred_effects) = deferred_step else {
        panic!("opened deferred target unexpectedly idled")
    };
    runtime
        .take_effect_ownership(deferred_effects.len())
        .expect("consume deferred target effects");
    let target_terminals = runtime.take_leader_wire_runtime_terminals();
    assert_eq!(target_terminals.len(), 1);
    let blocked_step = runtime
        .step(deadline)
        .expect("blocked peer input runs normally after target retirement");
    let blocked_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("blocked peer input retains its independent FIFO owner");
    assert_eq!(blocked_scheduling.selected, RuntimeSelectedOwnerKind::Fifo);
    let RuntimeStep::Advanced(blocked_effects) = blocked_step else {
        panic!("released peer input unexpectedly idled")
    };
    runtime
        .take_effect_ownership(blocked_effects.len())
        .expect("consume released peer effects");
    let blocked_terminals = runtime.take_leader_wire_runtime_terminals();
    let [blocked_terminal] = blocked_terminals.as_slice() else {
        panic!("released peer input emits exactly its own terminal")
    };
    let blocked_terminal_receipt = match blocked_terminal {
        LeaderWireRuntimeTerminal::Volatile(receipt)
        | LeaderWireRuntimeTerminal::Producer {
            runtime: receipt, ..
        } => receipt,
    };
    assert_eq!(blocked_terminal_receipt, &blocked_receipt);
    assert!(runtime.leader_wire_runtime_receipts.is_empty());
    assert!(runtime.deferred_lifecycle_ownership.is_empty());
    assert!(runtime.fence_retry_blocked_fifo_owners.is_empty());
    assert_eq!(runtime.queued_commands(), 0);
    assert!(!runtime.fail_closed);
}
#[test]
fn post_cut_old_logical_replay_cannot_overtake_fenced_busy_deferred_target() {
    let directory = TempDir::new().expect("temporary post-cut replay runtime directory");
    let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
        &directory,
        RuntimeQueueConfig::new(8, 1, 1),
        Some(0),
    );
    let replay = signed_runtime_proposal(&context, &keys, 0xDA);
    let wire::ConsensusMessageV2Payload::Proposal(replay_proposal) = &replay.payload else {
        unreachable!("replay fixture carries Proposal")
    };
    let replay_origin = context.roster
        [usize::try_from(replay_proposal.proposer).expect("small fixture proposer")]
    .validator
    .clone();
    let target = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
        signed_runtime_quorum_certificate_for_phase(
            &context,
            &keys,
            0xDB,
            wire::GlobalPhase::Prepare,
        ),
    ));
    let target_origin = context.roster[1].validator.clone();
    let (_leader_wire_directory, _leader_wire_ingress, ownerships) =
        preowned_leader_wire_ownerships(
            &context,
            &[
                (replay.clone(), replay_origin),
                (target.clone(), target_origin),
            ],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
    let [mut replay_ownership, target_ownership]: [FairV2IngressOwnershipEvidence; 2] = ownerships
        .try_into()
        .expect("fixture creates one old-logical replay and one target");
    let replay_logical_ordinal = replay_ownership
        .runtime_lifecycle_ordinal()
        .expect("replay retains its old logical position");
    let target_logical_ordinal = target_ownership
        .runtime_lifecycle_ordinal()
        .expect("target retains its logical position");
    assert!(replay_logical_ordinal < target_logical_ordinal);
    let target_source_physical_ordinal = target_ownership
        .physical_admission_ordinal()
        .expect("target owns a checked physical occurrence");
    let target_physical_cut = target_ownership
        .runtime_physical_cut()
        .expect("target owns a checked physical cut");
    // Model a reconnect which retained the replay's immutable logical
    // identity but acquired a fresh physical position after the target's
    // checked-dequeue cut.
    let replay_source_physical_ordinal =
        u64::try_from(target_physical_cut).expect("small fixture cut fits u64");
    replay_ownership.first.physical_admission_ordinal = replay_source_physical_ordinal;
    replay_ownership.latest.physical_admission_ordinal = replay_source_physical_ordinal;
    replay_ownership.runtime_physical_cut = target_physical_cut.checked_add(1);
    assert!(replay_ownership.validate_exact());
    let start = Instant::now();
    runtime
        .arm_live_clocks(start)
        .expect("arm runtime before opening the shared signing fence");
    let deadline = start + runtime.round_timeout();
    let timeout_step = runtime
        .step(deadline)
        .expect("absolute deadline opens TimeoutVote signing");
    runtime
        .take_last_scheduler_ownership()
        .expect("timeout retains exact scheduler ownership");
    let RuntimeStep::Advanced(timeout_effects) = timeout_step else {
        panic!("absolute deadline unexpectedly idled")
    };
    let timeout_ownership = runtime
        .take_effect_ownership(timeout_effects.len())
        .expect("TimeoutVote Sign retains its timeout root");
    let [timeout_ownership] = timeout_ownership.as_slice() else {
        panic!("TimeoutVote Sign has one exact owner")
    };
    let (sign_tag, signature_preimage) = match timeout_effects.as_slice() {
        [
            AdapterEffect::Sign {
                tag,
                request: SignRequest::TimeoutVote(vote),
            },
        ] => (*tag, vote.signature_preimage()),
        effects => panic!("unexpected timeout effects: {effects:?}"),
    };
    runtime
        .set_external_lifecycle_owners(vec![timeout_ownership.owner().clone()])
        .expect("publish pending TimeoutVote signer owner");
    runtime
        .enqueue_network_with_ingress_ownership(target.clone(), target_ownership)
        .expect("admit the target before the physical replay");
    runtime
        .set_ingress_physical_cut(
            target_physical_cut
                .checked_add(1)
                .expect("small target cut has a successor"),
        )
        .expect("later physical replay advances only the global high-watermark");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("target crosses into Busy-deferred ownership"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    let target_deferred_ordinal = runtime
        .driver()
        .all_deferred_admission_ordinals()
        .into_iter()
        .next()
        .expect("target owns one adapter-deferred ordinal");
    let target_deferred = &runtime.deferred_lifecycle_ownership[&target_deferred_ordinal];
    assert_eq!(
        target_deferred.source_physical_ordinal,
        Some(target_source_physical_ordinal)
    );
    assert_eq!(target_deferred.physical_cut, target_physical_cut);
    runtime
        .enqueue_network_with_ingress_ownership(replay.clone(), replay_ownership)
        .expect("admit the old-logical replay at its fresh physical position");
    assert!(matches!(
        runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("replay reaches a distinct Busy-deferred lane"),
        RuntimeStep::Advanced(ref effects) if effects.is_empty()
    ));
    assert_eq!(
        runtime.driver().all_deferred_admission_ordinals().len(),
        2,
        "different deferred classes retain independent bounded owners"
    );
    assert_eq!(
        runtime
            .eligible_deferred_admission_ordinals()
            .expect("pairwise physical selector remains exact"),
        BTreeSet::from([target_deferred_ordinal]),
        "the post-cut replay cannot reclaim its old logical priority"
    );
    let signature = Signature::new(keys[0].private_key(), &signature_preimage)
        .payload()
        .to_vec();
    runtime
        .enqueue_signature_with_owner(sign_tag, signature, timeout_ownership)
        .expect("enqueue the exact owned TimeoutVote completion");
    runtime
        .set_external_lifecycle_owners(Vec::new())
        .expect("retire pending signer after completion enqueue");
    let completion_step = runtime
        .step(deadline)
        .expect("the target-relative fence selector finds the exact completion");
    let completion_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("completion bypass retains scheduler evidence");
    assert_eq!(
        completion_scheduling.selected,
        RuntimeSelectedOwnerKind::FenceCompletion
    );
    assert_eq!(
        completion_scheduling.fence_predecessor_lifecycle_ordinal,
        Some(target_logical_ordinal)
    );
    assert_eq!(completion_scheduling.validate_exact(), Ok(()));
    let RuntimeStep::Advanced(completion_effects) = completion_step else {
        panic!("exact fence completion unexpectedly idled")
    };
    runtime
        .take_effect_ownership(completion_effects.len())
        .expect("consume completion effect ownership");
    let target_step = runtime
        .step(deadline)
        .expect("the pre-cut target owns service before the replay");
    let target_scheduling = runtime
        .take_last_scheduler_ownership()
        .expect("target service retains scheduler evidence");
    let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) = &target_scheduling.candidate
    else {
        panic!("expected exact deferred target ownership")
    };
    assert_eq!(
        candidate.lifecycle_ownership.physical_cut,
        target_physical_cut
    );
    assert_eq!(
        candidate.lifecycle_ownership.source_physical_ordinal,
        Some(target_source_physical_ordinal)
    );
    assert_eq!(
        candidate
            .ingress_ownership
            .as_ref()
            .expect("target retains authenticated provenance")
            .runtime_bytes
            .as_ref(),
        target.encode().as_slice(),
        "the selected deferred occurrence is the target, not the replay"
    );
    assert_eq!(target_scheduling.validate_exact(), Ok(()));
    let RuntimeStep::Advanced(target_effects) = target_step else {
        panic!("exact deferred target unexpectedly idled")
    };
    runtime
        .take_effect_ownership(target_effects.len())
        .expect("consume target effect ownership");
    let _ = runtime.take_leader_wire_runtime_terminals();
}
