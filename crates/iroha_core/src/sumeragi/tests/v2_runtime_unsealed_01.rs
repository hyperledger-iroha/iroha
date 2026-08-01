
    #[test]
    fn real_adapter_fence_completion_bypasses_only_preowned_fenced_fifo() {
        let directory = TempDir::new().expect("temporary preowned-fence runtime directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let first =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xD8),
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

        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("the later duplicate cannot cross the active signing fence"),
            RuntimeStep::Idle
        ));
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
        predecessor.owner.projection_hash =
            runtime_lifecycle_owner_projection_hash(&predecessor.owner);
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
        let RuntimeSelectedCandidateOwnership::Exact(second_candidate) =
            &second_scheduling.candidate
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
        let target =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xDB),
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
        let [mut replay_ownership, target_ownership]: [FairV2IngressOwnershipEvidence; 2] =
            ownerships
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
        let RuntimeSelectedCandidateOwnership::ExactDeferred(candidate) =
            &target_scheduling.candidate
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

    #[test]
    fn real_adapter_signature_completion_precedes_deferred_timeout_and_newer_ingress() {
        let directory = TempDir::new().expect("temporary real-adapter ordering directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime after adapter startup");

        // Refresh the derived clock before the signer becomes busy. This keeps
        // the absolute deadline and retransmission deadline independent in the
        // ordering trace below.
        let before_timeout = start + Duration::from_secs(9);
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("service pre-fence retransmission"),
            RuntimeStep::Advanced(_)
        ));

        let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
        runtime
            .enqueue_network(proposal.clone())
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch authenticated proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
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

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue reconstructed body");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
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
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-body completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch durable-body completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        runtime
            .enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validated-body completion");
        let validation_step = runtime
            .step(before_timeout)
            .expect("dispatch validated-body completion");
        runtime
            .take_last_scheduler_ownership()
            .expect("validation retains exact scheduler ownership");
        let RuntimeStep::Advanced(validation_effects) = validation_step else {
            panic!("validation dispatch unexpectedly idle")
        };
        let prepare_effect_ownership = runtime
            .take_effect_ownership(validation_effects.len())
            .expect("Prepare signature request retains its lifecycle owner");
        let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
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
            effects => panic!("unexpected validation effects: {effects:?}"),
        };
        assert_eq!(prepare_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
            .expect("publish pending Prepare signer owner");

        // The body pipeline leaves the fair-ingress cursor at Progress. An
        // exact authenticated retransmission is consumed below the reducer
        // fence and advances that cursor normally, so Completion owns the
        // first slot once the signature and newer ingress arrive together.
        runtime
            .enqueue_network(proposal)
            .expect("enqueue exact authenticated retransmission");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("coalesce exact authenticated retransmission"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.ingress.next_class, CommandClass::Completion);

        let deadline = start + runtime.round_timeout();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("deliver absolute timeout through the real adapter"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            !runtime.driver().deferred_work_is_serviceable(),
            "the exact Prepare signature still fences the Busy-deferred timeout"
        );

        let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature_with_owner(
                prepare_sign_tag,
                prepare_signature,
                &prepare_effect_ownership[0],
            )
            .expect("enqueue exact Prepare signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire pending Prepare signer owner after completion enqueue");
        runtime
            .enqueue_network(signed_runtime_proposal(&context, &keys, 0xE2))
            .expect("enqueue newer authenticated ingress");
        assert_eq!(runtime.queued_commands(), 2);

        let prepare_broadcast = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("signature completion owns the first serialized turn");
        assert!(matches!(
            prepare_broadcast,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Broadcast(message)]
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::Vote(vote)
                                if vote.phase == wire::GlobalPhase::Prepare
                                    && vote.round == manifest.round
                                    && vote.subject == manifest.subject
                        )
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "newer ingress remains owned after signature completion"
        );

        let timeout_macro_step = runtime
            .step_and_take_scheduler_ownership_for_test(deadline)
            .expect("service exactly one older Busy-deferred timeout transition");
        assert!(matches!(
            timeout_macro_step,
            RuntimeStep::Advanced(ref effects)
                if matches!(
                    effects.as_slice(),
                    [AdapterEffect::Sign {
                        request: SignRequest::TimeoutVote(vote),
                        ..
                    }] if vote.round == manifest.round
                )
        ));
        assert_eq!(
            runtime.queued_commands(),
            1,
            "one deferred macro-step cannot concatenate newer ingress"
        );

        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(deadline)
                .expect("dispatch newer ingress"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ReportEquivocation { .. }])
        ));
        assert_eq!(runtime.queued_commands(), 0);

        let next_retransmission = before_timeout + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(next_retransmission)
                .expect("make the next periodic scheduling decision"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert_eq!(runtime.retransmit_started_at, next_retransmission);
    }

    #[test]
    fn real_adapter_fence_completion_breaks_pre_and_post_timeout_retransmit_debt() {
        let directory = TempDir::new().expect("temporary real-adapter ordering directory");
        let (mut runtime, context, keys) = authenticated_network_runtime_with_local_validator(
            &directory,
            RuntimeQueueConfig::new(8, 1, 1),
            Some(0),
        );
        let start = Instant::now();
        runtime
            .arm_live_clocks(start)
            .expect("arm runtime after adapter startup");

        // Service the first periodic episode before the signer becomes busy.
        // Every later tick in this view reconstructs this exact cached root,
        // including its immutable early lifecycle ordinal.
        let before_timeout = start + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("service pre-fence retransmission"),
            RuntimeStep::Advanced(_)
        ));

        let proposal = signed_runtime_proposal(&context, &keys, 0xE1);
        runtime
            .enqueue_network(proposal)
            .expect("enqueue authenticated proposal");
        let proposal_effects = match runtime
            .step_and_take_scheduler_ownership_for_test(before_timeout)
            .expect("dispatch authenticated proposal")
        {
            RuntimeStep::Advanced(effects) => effects,
            RuntimeStep::Idle => panic!("proposal dispatch unexpectedly idle"),
        };
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

        runtime
            .enqueue_body_available(tag, manifest.clone())
            .expect("enqueue reconstructed body");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
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
        runtime
            .enqueue_body_stored(tag, manifest.round, manifest.subject, durable.clone())
            .expect("enqueue durable-body completion");
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(before_timeout)
                .expect("dispatch durable-body completion"),
            RuntimeStep::Advanced(ref effects)
                if matches!(effects.as_slice(), [AdapterEffect::ValidateBody { .. }])
        ));
        runtime
            .enqueue_validation_succeeded(
                tag,
                manifest.round,
                manifest.subject,
                ValidatedBodyReceipt::for_test(durable),
            )
            .expect("enqueue validated-body completion");
        let validation_step = runtime
            .step(before_timeout)
            .expect("dispatch validated-body completion");
        runtime
            .take_last_scheduler_ownership()
            .expect("validation macro-step retains exact scheduler ownership");
        let RuntimeStep::Advanced(validation_effects) = validation_step else {
            panic!("validation dispatch unexpectedly idled")
        };
        let prepare_effect_ownership = runtime
            .take_effect_ownership(validation_effects.len())
            .expect("Prepare signature request retains its lifecycle owner");
        let (prepare_sign_tag, prepare_signature_preimage) = match validation_effects.as_slice() {
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
            effects => panic!("unexpected validation effects: {effects:?}"),
        };
        assert_eq!(prepare_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![prepare_effect_ownership[0].owner().clone()])
            .expect("publish the pending Prepare signer owner");

        // The second periodic episode is still before the absolute deadline.
        // Its cached root predates the proposal lifecycle, reaches the reducer
        // while Prepare signing is fenced, and becomes the oldest
        // Busy-deferred owner.
        let second_retransmission = before_timeout + runtime.retransmit_interval();
        assert!(second_retransmission < start + runtime.round_timeout());
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("defer the pre-deadline second retransmission"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            !runtime.driver().deferred_work_is_serviceable(),
            "the exact Prepare signature still fences retransmission debt"
        );
        assert!(
            runtime.retransmit_owner.is_none(),
            "the cached retransmission root must not retain a second runtime alias"
        );

        let prepare_signature = Signature::new(keys[0].private_key(), &prepare_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature(prepare_sign_tag, prepare_signature.clone())
            .expect("enqueue an independently rooted signature callback");
        runtime
            .enqueue_signature_with_owner(
                prepare_sign_tag,
                prepare_signature,
                &prepare_effect_ownership[0],
            )
            .expect("enqueue exact Prepare signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire the pending Prepare signer owner after completion enqueue");
        assert_eq!(runtime.queued_commands(), 2);

        let prepare_broadcast = runtime
            .step(second_retransmission)
            .expect("owned Prepare completion opens the retransmission fence");
        let prepare_bypass = runtime
            .take_last_scheduler_ownership()
            .expect("fence completion retains exact scheduler ownership");
        assert_eq!(
            prepare_bypass.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert!(prepare_bypass.fence_completion_bypass);
        assert!(
            prepare_bypass
                .fence_predecessor_lifecycle_ordinal
                .is_some_and(|predecessor| {
                    let RuntimeSelectedCandidateOwnership::Exact(candidate) =
                        &prepare_bypass.candidate
                    else {
                        return false;
                    };
                    predecessor < candidate.lifecycle_ordinal
                })
        );
        assert!(prepare_bypass.validate_exact().is_ok());
        let mut local_cut_mutation = prepare_bypass.clone();
        let mutated_local_cut = local_cut_mutation
            .fence_predecessor_ownership
            .as_ref()
            .expect("local retransmit fence carries its exact wrapper")
            .physical_cut
            .checked_add(1)
            .expect("small local cut has a successor");
        local_cut_mutation
            .fence_predecessor_ownership
            .as_mut()
            .expect("local retransmit fence carries its exact wrapper")
            .physical_cut = mutated_local_cut;
        local_cut_mutation.projection_hash = runtime_scheduler_projection_hash(&local_cut_mutation);
        assert_eq!(
            local_cut_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the adapter-private seal rejects a coherently rehashed local cut"
        );
        let mut local_rank_mutation = prepare_bypass.clone();
        let mutated_local_rank = {
            let wrapper = local_rank_mutation
                .fence_predecessor_ownership
                .as_mut()
                .expect("local retransmit fence carries its exact wrapper");
            let mutated = wrapper
                .owner
                .lifecycle_ordinal
                .checked_add(1)
                .expect("small local lifecycle rank has a successor");
            wrapper.owner.lifecycle_ordinal = mutated;
            wrapper.owner.causal_origin.root_lifecycle_ordinal = Some(mutated);
            wrapper.owner.causal_origin.projection_hash =
                runtime_candidate_causal_origin_projection_hash(&wrapper.owner.causal_origin);
            wrapper.owner.projection_hash = runtime_lifecycle_owner_projection_hash(&wrapper.owner);
            mutated
        };
        local_rank_mutation.fence_predecessor_lifecycle_ordinal = Some(mutated_local_rank);
        local_rank_mutation.projection_hash =
            runtime_scheduler_projection_hash(&local_rank_mutation);
        assert_eq!(
            local_rank_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the adapter-private seal rejects a coherently rehashed local logical rank"
        );
        let mut foreign_seal_mutation = prepare_bypass.clone();
        let foreign_wrapper = foreign_seal_mutation
            .fence_predecessor_ownership
            .as_mut()
            .expect("local retransmit fence carries its exact wrapper");
        foreign_wrapper.runtime_seal = DeferredRuntimeOwnershipSeal::for_test(
            foreign_wrapper.deferred_admission_ordinal,
            foreign_wrapper.owner.causal_origin().lifecycle_key.clone(),
            foreign_wrapper.owner.lifecycle_ordinal(),
            false,
            foreign_wrapper.source_physical_ordinal,
            foreign_wrapper.physical_cut,
        );
        foreign_seal_mutation.projection_hash =
            runtime_scheduler_projection_hash(&foreign_seal_mutation);
        assert_eq!(
            foreign_seal_mutation.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "a same-number foreign capability cannot replace the exact adapter seal"
        );
        let RuntimeStep::Advanced(prepare_broadcasts) = prepare_broadcast else {
            panic!("Prepare fence completion unexpectedly idled")
        };
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
            .expect("test executor consumes Prepare broadcast ownership");
        assert!(
            runtime.retransmit_owner.is_none(),
            "the deferred retransmission remains the sole cached-root owner"
        );
        assert_eq!(
            runtime.queued_commands(),
            1,
            "the independently rooted callback cannot use the dependency bypass"
        );

        // Once the fence is open, the exact older retransmission debt runs and
        // rebroadcasts the newly published Prepare vote. Other finite deferred
        // work and the independently rooted callback then drain normally.
        let retransmit_retry = runtime
            .step_and_take_scheduler_ownership_for_test(second_retransmission)
            .expect("service older pre-deadline retransmission debt");
        assert!(matches!(
            retransmit_retry,
            RuntimeStep::Advanced(ref effects)
                if effects.iter().any(|effect| matches!(
                    effect,
                    AdapterEffect::Broadcast(message)
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::Vote(vote)
                                if vote.phase == wire::GlobalPhase::Prepare
                                    && vote.round == manifest.round
                        )
                ))
        ));
        assert_eq!(
            prepare_bypass.validate_exact(),
            Ok(()),
            "immutable fence evidence remains valid after its target is later claimed"
        );
        while runtime.driver().deferred_work_is_serviceable() {
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("drain finite adapter debt after Prepare completion");
        }
        while runtime.queued_commands() != 0 {
            runtime
                .step_and_take_scheduler_ownership_for_test(second_retransmission)
                .expect("drain non-bypassing completion normally");
        }
        assert!(
            !runtime.fail_closed,
            "an independently rooted completion remains a recoverable ordinary FIFO occurrence"
        );

        // Absolute timeout remains one-shot after the pre-deadline dependency
        // cycle has drained. A drained cached retransmission root is not
        // replenished ahead of this still-unemitted timeout.
        let deadline = start + runtime.round_timeout();
        let timeout_macro_step = runtime
            .step(deadline)
            .expect("deliver the absolute timeout through the real adapter");
        runtime
            .take_last_scheduler_ownership()
            .expect("timeout macro-step retains exact scheduler ownership");
        let RuntimeStep::Advanced(timeout_effects) = timeout_macro_step else {
            panic!("absolute timeout unexpectedly idled")
        };
        let timeout_effect_ownership = runtime
            .take_effect_ownership(timeout_effects.len())
            .expect("timeout signature request retains its lifecycle owner");
        let (timeout_sign_tag, timeout_signature_preimage) = match timeout_effects.as_slice() {
            [
                AdapterEffect::Sign {
                    tag,
                    request: SignRequest::TimeoutVote(vote),
                },
            ] if vote.round == manifest.round => (*tag, vote.signature_preimage()),
            effects => panic!("unexpected timeout effects: {effects:?}"),
        };
        assert_eq!(timeout_effect_ownership.len(), 1);
        runtime
            .set_external_lifecycle_owners(vec![timeout_effect_ownership[0].owner().clone()])
            .expect("publish the pending TimeoutVote signer owner");

        // The cached retransmission root becomes due again while TimeoutVote
        // signing is active. It is allowed one bounded turn and becomes
        // unserviceable Busy debt; it must not be resurrected over its later
        // exact completion on every subsequent call.
        let post_timeout_retransmission = deadline + runtime.retransmit_interval();
        assert!(matches!(
            runtime
                .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
                .expect("defer post-timeout retransmission behind signing"),
            RuntimeStep::Advanced(ref effects) if effects.is_empty()
        ));
        assert!(
            runtime.retransmit_owner.is_none(),
            "post-timeout deferred retransmission must not retain a runtime alias"
        );

        let timeout_signature = Signature::new(keys[0].private_key(), &timeout_signature_preimage)
            .payload()
            .to_vec();
        runtime
            .enqueue_signature_with_owner(
                timeout_sign_tag,
                timeout_signature,
                &timeout_effect_ownership[0],
            )
            .expect("enqueue exact TimeoutVote signature completion");
        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("retire the pending TimeoutVote signer owner after completion enqueue");
        let first_timeout_vote = runtime
            .step(post_timeout_retransmission)
            .expect("owned TimeoutVote completion opens the retransmission fence");
        let timeout_bypass = runtime
            .take_last_scheduler_ownership()
            .expect("TimeoutVote completion retains exact scheduler ownership");
        assert_eq!(
            timeout_bypass.selected,
            RuntimeSelectedOwnerKind::FenceCompletion
        );
        assert!(timeout_bypass.fence_completion_bypass);
        assert!(timeout_bypass.fence_predecessor_lifecycle_ordinal.is_some());
        assert!(timeout_bypass.validate_exact().is_ok());
        let RuntimeStep::Advanced(first_timeout_vote_effects) = first_timeout_vote else {
            panic!("TimeoutVote fence completion unexpectedly idled")
        };
        assert!(first_timeout_vote_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                        if vote.round == manifest.round
                )
        )));
        runtime
            .take_effect_ownership(first_timeout_vote_effects.len())
            .expect("test executor consumes first TimeoutVote ownership");
        assert!(
            runtime.retransmit_owner.is_none(),
            "the deferred retransmission remains the sole post-timeout cached-root owner"
        );

        // Treat the first TimeoutVote broadcast as lost. The exact overdue
        // retransmission debt is still present and must rebroadcast it on the
        // next serialized turn rather than being permanently suppressed after
        // the absolute deadline.
        let timeout_vote_retry = runtime
            .step_and_take_scheduler_ownership_for_test(post_timeout_retransmission)
            .expect("rebroadcast a lost first TimeoutVote");
        assert!(matches!(
            timeout_vote_retry,
            RuntimeStep::Advanced(ref effects)
                if effects.iter().any(|effect| matches!(
                    effect,
                    AdapterEffect::Broadcast(message)
                        if matches!(
                            &message.payload,
                            wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                                if vote.round == manifest.round
                        )
                ))
        ));
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            runtime
                .driver()
                .all_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
        assert!(runtime.retransmit_owner.is_none());

        // A later periodic tick remains armed after the one-shot timeout and
        // continues broadcasting the published TimeoutVote.
        let later_post_timeout_tick = post_timeout_retransmission + runtime.retransmit_interval();
        let later_retry = runtime
            .step(later_post_timeout_tick)
            .expect("service a later post-timeout periodic tick");
        let later_retry_owner = runtime
            .take_last_scheduler_ownership()
            .expect("later periodic tick retains scheduler ownership");
        assert_eq!(
            later_retry_owner.selected,
            RuntimeSelectedOwnerKind::PeriodicTimer
        );
        assert!(later_retry_owner.validate_exact().is_ok());
        let RuntimeStep::Advanced(later_retry_effects) = later_retry else {
            panic!("later post-timeout periodic tick unexpectedly idled")
        };
        assert!(later_retry_effects.iter().any(|effect| matches!(
            effect,
            AdapterEffect::Broadcast(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                        if vote.round == manifest.round
                )
        )));
        runtime
            .take_effect_ownership(later_retry_effects.len())
            .expect("test executor consumes later TimeoutVote retry ownership");
        assert_eq!(runtime.queued_commands(), 0);
        assert!(
            runtime
                .driver()
                .all_deferred_admission_ordinals()
                .is_empty()
        );
        assert!(runtime.deferred_lifecycle_ownership.is_empty());
    }

    #[test]
    fn round_timeout_grows_linearly_by_view_without_wrapping() {
        let base = Duration::from_secs(10);
        assert_eq!(round_timeout_for_view(base, 0), base);
        assert_eq!(round_timeout_for_view(base, 1), Duration::from_secs(20));
        assert_eq!(round_timeout_for_view(base, 7), Duration::from_secs(80));
        assert_eq!(
            round_timeout_for_view(Duration::new(1, 500_000_000), 1),
            Duration::from_secs(3),
        );

        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX - 1),
            Duration::from_secs(u64::MAX)
        );
        assert_eq!(
            round_timeout_for_view(Duration::from_secs(1), u64::MAX),
            Duration::MAX
        );
        assert_eq!(round_timeout_for_view(Duration::MAX, 1), Duration::MAX);
    }

    #[test]
    fn recovered_nonzero_view_uses_scaled_timeout_from_live_arm() {
        let constructed_at = Instant::now();
        let armed_at = constructed_at + Duration::from_secs(500);
        let recovered = tag(4);
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(recovered),
            constructed_at,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            Vec::new(),
        )
        .expect("open recovered runtime");

        runtime
            .arm_live_clocks(armed_at)
            .expect("arm after recovered startup");
        assert_eq!(runtime.round_timeout(), Duration::from_secs(50));
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(49));
        assert!(runtime.driver.timeouts.is_empty());
        let _ =
            runtime.step_and_take_scheduler_ownership_for_test(armed_at + Duration::from_secs(50));
        assert_eq!(runtime.driver.timeouts, vec![recovered]);
    }

    #[test]
    fn class_aware_ingress_is_bounded_and_reserves_progress_and_completion_slots() {
        let start = Instant::now();
        let initial = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(initial),
            start,
            RuntimeQueueConfig::new(4, 1, 1),
        );
        assert_eq!(runtime.remaining_completion_capacity(), 4);

        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 3);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Normal,
            FakeCommand::record(2),
        )
        .unwrap();
        assert_eq!(runtime.remaining_completion_capacity(), 2);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Normal,
                FakeCommand::record(99)
            ),
            Err(EnqueueError::ReservedCapacity)
        );
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("reserved progress slot");
        assert_eq!(runtime.remaining_completion_capacity(), 1);
        enqueue_fake(
            &mut runtime,
            initial,
            CommandClass::Completion,
            FakeCommand::record(4),
        )
        .expect("reserved completion slot");
        assert_eq!(runtime.remaining_completion_capacity(), 0);
        assert_eq!(runtime.queued_commands(), 4);
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                initial,
                CommandClass::Completion,
                FakeCommand::record(5)
            ),
            Err(EnqueueError::Full)
        );

        for offset in 0..4 {
            let _ = runtime
                .step_and_take_scheduler_ownership_for_test(start + Duration::from_millis(offset));
        }
        assert_eq!(
            runtime.driver.delivered,
            vec![(initial, 4), (initial, 3), (initial, 1), (initial, 2)]
        );
    }

    #[test]
    fn scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("normal owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(9),
        )
        .expect("progress owner fits");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("FIFO dispatch retains exact scheduler ownership")
            .clone();
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Fifo);
        assert_eq!(evidence.round_tag, owner_tag);
        assert_eq!(evidence.queue_before.len, 2);
        assert_eq!(evidence.queue_after.len, 1);
        assert_eq!(
            evidence.queue_before.service_cursor,
            SERVICE_CLASS_COMPLETION
        );
        assert_eq!(evidence.queue_after.service_cursor, SERVICE_CLASS_NORMAL);
        assert_eq!(evidence.queue_before.max_service_debt, 0);
        assert_eq!(evidence.queue_after.max_service_debt, 1);
        assert!(evidence.live_mode);
        assert!(!evidence.timeout_due);
        assert!(!evidence.periodic_timer_due);
        assert!(evidence.fifo_ready);
        assert!(!evidence.completion_ready);
        assert!(evidence.progress_ready);
        assert!(evidence.normal_ready);
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &evidence.candidate else {
            panic!("FIFO dispatch must carry one exact command candidate");
        };
        assert_eq!(
            candidate.identity,
            FakeCommand::record(9)
                .exact_runtime_command_identity()
                .digest()
        );
        assert_eq!(candidate.kind, RuntimeCommandKind::Test);
        assert_eq!(candidate.class, SERVICE_CLASS_PROGRESS);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 2);
        assert_eq!(candidate.lifecycle_ordinal, 2);
        assert_eq!(candidate.causal_origin.root_lifecycle_ordinal, Some(2));
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.eligible_skips_before, 0);
        assert_eq!(candidate.eligible_skips_after, 0);
        assert_eq!(evidence.validate_exact(), Ok(()));

        let rejected = |mutated: RuntimeSchedulerOwnershipEvidence| {
            assert_eq!(
                mutated.validate_exact(),
                Err(RuntimeSchedulerEvidenceError::InvalidProjection)
            );
        };

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.identity.canonical_hash = iroha_crypto::Hash::new([0xFF]);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.identity = FakeCommand::record(42)
            .exact_runtime_command_identity()
            .digest();
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.kind = RuntimeCommandKind::Authenticated;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.class = SERVICE_CLASS_NORMAL;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.tag = tag(99);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.tag = tag(99);
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = 0;
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.lifecycle_ordinal = candidate
            .lifecycle_ordinal
            .checked_add(1)
            .expect("small test lifecycle rank has a successor");
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        let replacement_origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            candidate.tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"coherently-rehashed-causal-root",
        );
        candidate.causal_origin =
            RuntimeLifecycleOwner::new(replacement_origin, candidate.lifecycle_ordinal)
                .expect("replacement causal root retains the same logical ordinal")
                .causal_origin()
                .clone();
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.admission_ordinal = candidate
            .lifecycle_ordinal
            .checked_sub(1)
            .expect("fresh FIFO lifecycle rank has a nonzero predecessor");
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.fifo_position = 0;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.service_cursor = SERVICE_CLASS_COMPLETION;
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_after.max_service_debt =
            evidence.queue_before.max_service_debt.saturating_add(2);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.queue_before.service_cursor = SERVICE_CLASS_NONE;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.timeout_due = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.progress_ready = false;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence.clone();
        mutated.fifo_owed_after = true;
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);

        let mut mutated = evidence;
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut mutated.candidate else {
            unreachable!();
        };
        candidate.eligible_skips_before = 1;
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        mutated.projection_hash = runtime_scheduler_projection_hash(&mutated);
        rejected(mutated);
    }

    #[test]
    fn scheduler_queue_seal_rejects_valid_same_wire_ingress_carrier_substitution() {
        let directory = TempDir::new().expect("temporary scheduler-ingress-seal directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(4, 1, 1));
        let now = Instant::now();
        runtime
            .arm_live_clocks(now)
            .expect("arm runtime before authenticated scheduler selection");
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                signed_runtime_quorum_certificate(&context, &keys, 0xA7),
            ));
        let original_source = PeerId::new(keys[0].public_key().clone());
        let replacement_source = PeerId::new(keys[1].public_key().clone());
        let replacement_ingress = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &message,
            fair_network_ownership(&message, replacement_source),
        )
        .expect("independent same-wire carrier has exact runtime ownership");
        assert!(replacement_ingress.validate_frozen_physical());

        runtime
            .enqueue_network_with_ingress_ownership(
                message.clone(),
                fair_network_ownership(&message, original_source),
            )
            .expect("original authenticated carrier enters the runtime FIFO");
        assert!(matches!(runtime.step(now), Ok(RuntimeStep::Advanced(_))));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("authenticated FIFO selection retains exact scheduler ownership")
            .clone();
        assert_eq!(evidence.validate_exact(), Ok(()));
        let RuntimeSelectedCandidateOwnership::Exact(original) = &evidence.candidate else {
            panic!("authenticated FIFO dispatch must retain one exact candidate")
        };
        let original_ingress = original
            .ingress_ownership
            .as_ref()
            .expect("authenticated candidate retains its full ingress carrier");
        assert_ne!(
            replacement_ingress.projection_hash, original_ingress.projection_hash,
            "independent sources have distinct complete ownership projections"
        );
        assert_eq!(
            runtime_ingress_causal_origin_projection_hash(&replacement_ingress),
            runtime_ingress_causal_origin_projection_hash(original_ingress),
            "equal aggregate certificates retain one route-neutral logical identity"
        );
        assert_eq!(
            replacement_ingress.earliest_physical_carrier(),
            original_ingress.earliest_physical_carrier(),
            "the independent test queues deliberately assign the same valid physical shape"
        );
        assert_eq!(
            replacement_ingress.earliest_lifecycle_ordinal(),
            original_ingress.earliest_lifecycle_ordinal(),
            "the replacement is rank-compatible before the private selection check"
        );

        let mut substituted = evidence;
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = &mut substituted.candidate else {
            unreachable!();
        };
        candidate.ingress_ownership = Some(replacement_ingress);
        assert!(runtime_fifo_candidate_ingress_is_exact(candidate));
        candidate.projection_hash = runtime_fifo_candidate_projection_hash(candidate);
        substituted.projection_hash = runtime_scheduler_projection_hash(&substituted);
        assert_eq!(
            substituted.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "the queue-private seal rejects a valid same-wire full-carrier substitution after every public projection is recomputed"
        );
    }

    #[test]
    fn full_lane_retryable_backpressure_restores_and_services_exact_fifo_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(1));
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(3, 1, 1));
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("oldest retryable owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(2),
        )
        .expect("later completion owner fits");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Progress,
            FakeCommand::record(3),
        )
        .expect("later progress owner fills the lane");
        assert_eq!(runtime.ingress.remaining_capacity(), 0);
        let original = runtime
            .ingress
            .commands
            .front()
            .expect("oldest physical owner is present")
            .clone();

        assert!(matches!(
            runtime.step(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retry turn retains typed scheduler ownership")
            .clone();
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::FifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, 3);
        assert_eq!(evidence.queue_after.len, 3);
        assert_eq!(evidence.validate_exact(), Ok(()));
        let restored = runtime
            .ingress
            .commands
            .front()
            .expect("retry restores the original physical owner");
        assert_eq!(restored.tag, original.tag);
        assert_eq!(restored.class, original.class);
        assert_eq!(restored.identity, original.identity);
        assert_eq!(restored.admission_ordinal, original.admission_ordinal);
        assert_eq!(restored.lifecycle_ordinal, original.lifecycle_ordinal);
        assert_eq!(restored.causal_origin, original.causal_origin);
        assert_eq!(runtime.driver.delivered, Vec::new());

        let mut weakened = evidence.clone();
        weakened.selected = RuntimeSelectedOwnerKind::Fifo;
        weakened.projection_hash = runtime_scheduler_projection_hash(&weakened);
        assert_eq!(
            weakened.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "an equal-length retry cannot be relabelled as completed FIFO service"
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.ingress.len(), 2);
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .and_then(|queued| queued.command.record),
            Some(2),
            "later Completion work cannot overtake the retained lifecycle"
        );
    }

    #[test]
    fn retryable_backpressure_restores_the_exact_recovery_fifo_owner_once() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        assert!(driver.retry_once.insert(7));
        let (mut runtime, _) = SerializedV2Runtime::with_driver(
            driver,
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(4, 1, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery owner fits");
        let original_owner = runtime
            .ingress
            .commands
            .front()
            .expect("recovery owner is present")
            .lifecycle_owner()
            .expect("recovery owner is exact");

        assert!(matches!(
            runtime.step_recovery(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.is_empty()
        ));
        let evidence = runtime
            .last_scheduler_ownership()
            .expect("retrying recovery retains scheduler ownership");
        assert_eq!(
            evidence.selected,
            RuntimeSelectedOwnerKind::RecoveryFifoRetryRetained
        );
        assert_eq!(evidence.queue_before.len, evidence.queue_after.len);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert_eq!(
            runtime
                .ingress
                .commands
                .front()
                .expect("recovery retry remains physically admitted")
                .lifecycle_owner()
                .expect("restored recovery owner is exact"),
            original_owner
        );
        assert!(runtime.take_last_scheduler_ownership().is_some());
        assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));

        assert!(matches!(
            runtime.step_recovery_and_take_scheduler_ownership_for_test(start),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 7)]);
        assert_eq!(runtime.queued_commands(), 0);
    }

    #[test]
    fn adapter_command_identity_is_derived_from_exact_immutable_payload() {
        let owner_tag = tag(4);
        let command = AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x33]);
        let expected = command.exact_runtime_command_identity();
        let shared = expected.clone();
        assert!(Arc::ptr_eq(
            &expected.canonical_bytes,
            &shared.canonical_bytes
        ));
        assert_ne!(
            expected,
            AdapterCommand::SignatureCompleted(vec![0x11, 0x22, 0x34])
                .exact_runtime_command_identity()
        );

        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(4, 1, 1));
        ingress
            .enqueue(TaggedCommand::new(
                owner_tag,
                CommandClass::Completion,
                command,
                Instant::now(),
            ))
            .expect("exact adapter command fits completion capacity");
        let (_, candidate) = ingress
            .pop_next_with_ownership()
            .expect("adapter command retains its admission ordinal")
            .expect("adapter command owns the selected FIFO occurrence");
        assert_eq!(candidate.identity, expected.digest());
        assert_eq!(candidate.kind, RuntimeCommandKind::SignatureCompleted);
        assert_eq!(candidate.class, SERVICE_CLASS_COMPLETION);
        assert_eq!(candidate.tag, owner_tag);
        assert_eq!(candidate.admission_ordinal, 1);
        assert_eq!(candidate.fifo_position, 0);
    }

    #[test]
    fn scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut idle = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(idle.step(start), Ok(RuntimeStep::Idle)));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Idle)
        );
        let mut nonempty_debt_on_empty_queue = idle
            .last_scheduler_ownership()
            .expect("idle branch retains its empty queue projection")
            .clone();
        nonempty_debt_on_empty_queue.queue_before.max_service_debt = 1;
        nonempty_debt_on_empty_queue.projection_hash =
            runtime_scheduler_projection_hash(&nonempty_debt_on_empty_queue);
        assert_eq!(
            nonempty_debt_on_empty_queue.validate_exact(),
            Err(RuntimeSchedulerEvidenceError::InvalidProjection),
            "a coherently rehashed empty queue cannot claim service debt"
        );
        assert!(idle.take_last_scheduler_ownership().is_some());

        assert!(matches!(
            idle.step(start + Duration::from_secs(2)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(idle.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            idle.step(start + Duration::from_secs(10)),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            idle.last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::Timeout)
        );

        let (mut recovery, _) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(6, 2, 1),
            Vec::new(),
        )
        .expect("construct unarmed recovery runtime");
        enqueue_fake(
            &mut recovery,
            owner_tag,
            CommandClass::Completion,
            FakeCommand::record(7),
        )
        .expect("recovery FIFO owner fits");
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryFifo)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .validate_exact(),
            Ok(())
        );
        assert!(
            !recovery
                .last_scheduler_ownership()
                .expect("recovery FIFO retains evidence")
                .live_mode
        );
        assert!(recovery.take_last_scheduler_ownership().is_some());
        assert!(matches!(
            recovery.step_recovery(start),
            Ok(RuntimeStep::Idle)
        ));
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::RecoveryIdle)
        );
        assert_eq!(
            recovery
                .last_scheduler_ownership()
                .expect("recovery idle retains evidence")
                .validate_exact(),
            Ok(())
        );

        let mut deferred_driver = FakeDriver::new(owner_tag);
        deferred_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut deferred = runtime(deferred_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(deferred.step(start), Ok(RuntimeStep::Advanced(_))));
        let evidence = deferred
            .last_scheduler_ownership()
            .expect("deferred dispatch retains its typed occurrence");
        assert_eq!(evidence.selected, RuntimeSelectedOwnerKind::Deferred);
        assert_eq!(evidence.validate_exact(), Ok(()));
        assert!(matches!(
            &evidence.candidate,
            RuntimeSelectedCandidateOwnership::ExactDeferred(candidate)
                if candidate.service.admission_ordinal == 0
                    && candidate.lifecycle_ownership.owner.lifecycle_ordinal() == 1
                    && candidate.service.validate_exact()
                    && candidate.ingress_ownership.is_none()
        ));

        let mut unavailable_driver = FakeDriver::new(owner_tag);
        unavailable_driver.deferred_identity_unavailable = true;
        unavailable_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut unavailable = runtime(unavailable_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(
            unavailable.step(start),
            Err(RuntimeError::FailClosed)
        ));
        assert!(unavailable.last_scheduler_ownership().is_none());
    }

    #[test]
    fn runtime_rejects_replayed_foreign_and_mutated_deferred_tokens() {
        let start = Instant::now();
        let owner_tag = tag(0);

        let mut replay_driver = FakeDriver::new(owner_tag);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        replay_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let replayed = DeferredServiceEvidence::completion_for_test(
            &replay_driver.deferred_admission_ordinals,
            owner_tag,
            2,
            DeferredPriority::Completion,
        );
        assert!(replayed.claim_adapter_service_for_test());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed.clone());
        replay_driver
            .deferred_evidence_overrides
            .push_back(replayed);
        let mut replay = runtime(replay_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(replay.step(start), Ok(RuntimeStep::Advanced(_))));
        assert!(replay.take_last_scheduler_ownership().is_some());
        assert!(matches!(replay.step(start), Err(RuntimeError::FailClosed)));

        let mut foreign_driver = FakeDriver::new(owner_tag);
        foreign_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let foreign_source = DeferredAdmissionOrdinalSource::new(0);
        let foreign_evidence = DeferredServiceEvidence::completion_for_test(
            &foreign_source,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(foreign_evidence.claim_adapter_service_for_test());
        foreign_driver
            .deferred_evidence_overrides
            .push_back(foreign_evidence);
        let mut foreign = runtime(foreign_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(foreign.step(start), Err(RuntimeError::FailClosed)));

        let mut mutated_driver = FakeDriver::new(owner_tag);
        mutated_driver
            .deferred_effects
            .push_back(vec![FakeEffect::other()]);
        let mut mutated = DeferredServiceEvidence::completion_for_test(
            &mutated_driver.deferred_admission_ordinals,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert!(mutated.claim_adapter_service_for_test());
        mutated.protected_progress = true;
        mutated_driver
            .deferred_evidence_overrides
            .push_back(mutated);
        let mut mutated = runtime(mutated_driver, start, RuntimeQueueConfig::new(6, 2, 1));
        assert!(matches!(mutated.step(start), Err(RuntimeError::FailClosed)));
    }

    #[test]
    fn runtime_rejects_driver_selection_outside_eligible_deferred_owner_set() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let ineligible = DeferredServiceEvidence::completion_for_test(
            &driver.deferred_admission_ordinals,
            owner_tag,
            1,
            DeferredPriority::Completion,
        );
        assert_eq!(ineligible.admission_ordinal, 0);
        assert!(ineligible.claim_adapter_service_for_test());
        driver.deferred_evidence_overrides.push_back(ineligible);
        driver.deferred_active_ordinals.insert(1);

        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"eligible-deferred-owner",
        );
        let owner = RuntimeLifecycleOwner::new(origin, 1)
            .expect("test target owns the global minimum lifecycle rank");
        let ownership = deferred_lifecycle_ownership_for_test(
            owner,
            1,
            RuntimeDispatchIngress::LocalOrCausal,
            None,
            runtime.ingress_physical_cut,
        )
        .expect("test target retains an exact runtime wrapper");
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(1, ownership)
                .is_none()
        );
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("the active target has one exact eligible owner"),
            BTreeSet::from([1])
        );

        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("deferred driver selected an ineligible admission owner")
        );
    }

    #[test]
    fn runtime_rejects_two_deferred_occurrences_for_one_logical_lifecycle() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut driver = FakeDriver::new(owner_tag);
        driver.deferred_effects.push_back(vec![FakeEffect::other()]);
        let mut runtime = runtime(driver, start, RuntimeQueueConfig::new(6, 2, 1));
        let origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::StartupRecovery,
            b"duplicate-deferred-logical-owner",
        );
        let owner = RuntimeLifecycleOwner::new(origin, 1)
            .expect("duplicate fixture owns one exact logical lifecycle");
        let physical_cut = runtime.ingress_physical_cut;
        let (first, second) = {
            let source = runtime.driver.deferred_admission_ordinal_source();
            let make = || {
                let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                    source,
                    owner.causal_origin().lifecycle_key.clone(),
                    owner.lifecycle_ordinal(),
                    false,
                    None,
                    physical_cut,
                );
                let ordinal = runtime_seal.admission_ordinal();
                let ownership = RuntimeDeferredLifecycleOwnership::new(
                    owner.clone(),
                    ordinal,
                    RuntimeDispatchIngress::LocalOrCausal,
                    None,
                    physical_cut,
                    runtime_seal,
                )
                .expect("each duplicate wrapper is independently well formed");
                (ordinal, ownership)
            };
            (make(), make())
        };
        for (ordinal, ownership) in [first, second] {
            runtime.driver.deferred_active_ordinals.insert(ordinal);
            assert!(
                runtime
                    .deferred_lifecycle_ownership
                    .insert(ordinal, ownership)
                    .is_none()
            );
        }

        assert!(matches!(
            runtime.eligible_deferred_admission_ordinals(),
            Err(EnqueueError::FailClosed)
        ));
        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert_eq!(runtime.driver.deferred_dispatches, 0);
        assert_eq!(
            runtime.fail_closed_reason.as_deref(),
            Some("deferred physical-cut lifecycle ownership was invalid")
        );
    }

    #[test]
    fn scheduler_owner_must_be_taken_before_a_later_step_can_enter() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut blocked_runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );

        assert!(matches!(blocked_runtime.step(start), Ok(RuntimeStep::Idle)));
        let first_projection_hash = blocked_runtime
            .last_scheduler_ownership()
            .expect("first idle selection retains a carrier")
            .projection_hash;

        let periodic_at = start + blocked_runtime.retransmit_interval();
        assert!(matches!(
            blocked_runtime.step(periodic_at),
            Err(RuntimeError::FailClosed)
        ));
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner")
        );
        blocked_runtime.latch_fail_closed("a later generic failure");
        assert_eq!(
            blocked_runtime.fail_closed_reason.as_deref(),
            Some("live scheduling began with an unconsumed scheduler owner"),
            "fail-closed diagnostics retain the first invariant violation"
        );
        let retained = blocked_runtime
            .last_scheduler_ownership()
            .expect("failed re-entry preserves the first unconsumed carrier");
        assert_eq!(retained.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(retained.projection_hash, first_projection_hash);

        let mut runtime = self::runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));

        let taken = runtime
            .take_last_scheduler_ownership()
            .expect("effect boundary takes the exact first occurrence");
        assert_eq!(taken.selected, RuntimeSelectedOwnerKind::Idle);
        assert_eq!(taken.validate_exact(), Ok(()));
        assert!(runtime.last_scheduler_ownership().is_none());

        assert!(matches!(
            runtime.step(periodic_at),
            Ok(RuntimeStep::Advanced(_))
        ));
        assert_eq!(
            runtime
                .take_last_scheduler_ownership()
                .map(|evidence| evidence.selected),
            Some(RuntimeSelectedOwnerKind::PeriodicTimer)
        );
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn checked_admission_reservation_rejection_preserves_and_reuses_the_owner() {
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(40);
        let rejected: Result<(), EnqueueError> =
            source.with_checked_reservation(1, |first, successor| {
                assert_eq!(first, 41);
                assert_eq!(successor, 42);
                Err(EnqueueError::FailClosed)
            });
        assert_eq!(rejected, Err(EnqueueError::FailClosed));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after rejected checked reservation"),
            Some(41),
            "a rejected checked admission cannot burn its prospective owner"
        );

        let admitted = source
            .with_checked_reservation(1, |first, successor| Ok((first, successor)))
            .expect("retry commits the same prospective owner");
        assert_eq!(admitted, (41, 42));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after committed retry"),
            Some(42)
        );
    }

    #[test]
    fn checked_ingress_rejection_preserves_dormant_owner_until_exact_retry() {
        let owner_tag = tag(0);
        let lifecycle_key = Hash::new(b"checked rejection dormant owner");
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(1);
        let mut ingress = BoundedIngress::with_lifecycle_ordinals(
            RuntimeQueueConfig::new(4, 1, 1),
            source.clone(),
        );
        let dormant = RuntimeDormantLocalFifoReservation::completion(lifecycle_key, 1, 9);
        ingress
            .install_dormant_local_fifo_reservations(vec![dormant.clone()])
            .expect("install one exact restart-dormant owner");
        let mirror_before = ingress.next_admission_ordinal;

        let rejected: Result<(), EnqueueError> =
            ingress.with_checked_admission_ordinal_range(1, |checked_ingress, first, successor| {
                assert_eq!((first, successor), (2, 3));
                assert!(
                    checked_ingress
                        .dormant_local_fifo_reservations
                        .contains(&dormant)
                );
                Err(EnqueueError::FailClosed)
            });
        assert_eq!(rejected, Err(EnqueueError::FailClosed));
        assert_eq!(ingress.next_admission_ordinal, mirror_before);
        assert!(ingress.dormant_local_fifo_reservations.contains(&dormant));
        assert!(ingress.commands.is_empty());
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after rejected dormant replacement"),
            Some(2)
        );

        ingress
            .enqueue(restored_fake_command(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                lifecycle_key,
                1,
                9,
            ))
            .expect("exact retry reuses and commits the rejected prospective ordinal");
        assert!(ingress.dormant_local_fifo_reservations.is_empty());
        assert_eq!(ingress.commands.len(), 1);
        assert_eq!(ingress.commands[0].admission_ordinal, Some(2));
        assert_eq!(ingress.commands[0].lifecycle_ordinal, Some(1));
        assert_eq!(ingress.next_admission_ordinal, Some(3));
        assert_eq!(
            source
                .next_ordinal_for_test()
                .expect("inspect source after exact dormant retry"),
            Some(3)
        );
    }

    #[test]
    fn checked_admission_reservation_exhaustion_never_enters_commit() {
        let source = RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 1);
        let commit_called = std::cell::Cell::new(false);
        for _ in 0..2 {
            let result: Result<(), EnqueueError> = source.with_checked_reservation(1, |_, _| {
                commit_called.set(true);
                Ok(())
            });
            assert_eq!(result, Err(EnqueueError::FailClosed));
            assert_eq!(
                source
                    .next_ordinal_for_test()
                    .expect("inspect exhausted checked source"),
                Some(u128::MAX),
                "exhaustion and retry must preserve the last prospective value"
            );
        }
        assert!(!commit_called.get());
    }

    #[test]
    fn admission_ordinal_exhaustion_fails_runtime_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.lifecycle_ordinals =
            RuntimeLifecycleOrdinalSource::after_high_watermark(u128::MAX - 2);
        runtime.ingress.next_admission_ordinal = Some(u128::MAX - 1);
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
        )
        .expect("the last ordinal with a representable successor is valid");
        assert_eq!(
            runtime.ingress.commands[0].admission_ordinal,
            Some(u128::MAX - 1)
        );
        let next_before_rejection = runtime.ingress.next_admission_ordinal;
        let source_before_rejection = runtime
            .ingress
            .lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect source before exhausted FIFO admission");
        assert_eq!(
            enqueue_fake(
                &mut runtime,
                owner_tag,
                CommandClass::Normal,
                FakeCommand::record(2),
            ),
            Err(EnqueueError::FailClosed)
        );
        assert!(runtime.fail_closed);
        assert_eq!(runtime.ingress.commands.len(), 1);
        assert_eq!(
            runtime.ingress.next_admission_ordinal,
            next_before_rejection
        );
        assert_eq!(
            runtime
                .ingress
                .lifecycle_ordinals
                .next_ordinal_for_test()
                .expect("inspect source after exhausted FIFO admission"),
            source_before_rejection,
            "failed FIFO admission cannot advance either ordinal representation"
        );
    }

    #[test]
    fn selected_owner_without_a_runtime_minted_ordinal_fails_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        runtime.ingress.commands.push_back(TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            start,
        ));

        assert!(matches!(runtime.step(start), Err(RuntimeError::FailClosed)));
        assert!(runtime.fail_closed);
        assert!(runtime.last_scheduler_ownership().is_none());
    }

    #[test]
    fn corrupt_cached_identity_and_rebound_origin_are_rejected_before_service() {
        let admitted_at = Instant::now();
        let owner_tag = tag(0);
        let mut ingress = BoundedIngress::new(RuntimeQueueConfig::new(6, 2, 1));
        let mut corrupt = TaggedCommand::new(
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(1),
            admitted_at,
        );
        corrupt.identity.canonical_hash = iroha_crypto::Hash::new(b"corrupt cached identity");
        assert_eq!(ingress.enqueue(corrupt), Err(EnqueueError::FailClosed));
        assert!(ingress.commands.is_empty());

        let root = FakeCommand::record(2);
        let mut origin =
            RuntimeCandidateCausalOrigin::mint(owner_tag, CommandClass::Normal, &root, None);
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            TaggedCommand::with_causal_origin(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(3),
                admitted_at,
                origin,
                8,
            ),
            Err(EnqueueError::FailClosed)
        ));
    }

    #[test]
    fn lifecycle_owner_constructor_rejects_a_conflicting_prebound_ordinal() {
        let owner_tag = tag(0);
        let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
            owner_tag,
            CommandClass::Progress,
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
            b"prebound-owner",
        );
        assert!(origin.bind_lifecycle_ordinal(7));
        assert!(matches!(
            RuntimeLifecycleOwner::new(origin.clone(), 8),
            Err(EnqueueError::FailClosed)
        ));
        let exact = RuntimeLifecycleOwner::new(origin, 7)
            .expect("the already-bound exact ordinal remains admissible");
        assert!(exact.validate_exact());
        assert_eq!(exact.lifecycle_ordinal(), 7);
    }

    #[test]
    fn runtime_physical_cut_is_monotone_and_regression_fails_closed() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(6, 2, 1),
        );
        assert_eq!(runtime.ingress_physical_cut, 1);
        runtime
            .set_ingress_physical_cut(4)
            .expect("receiver high-watermark advances");
        runtime
            .set_ingress_physical_cut(4)
            .expect("publishing the same high-watermark is idempotent");
        assert_eq!(runtime.ingress_physical_cut, 4);
        assert!(runtime.set_ingress_physical_cut(3).is_err());
        assert!(runtime.fail_closed);
        assert_eq!(runtime.ingress_physical_cut, 4);
    }

    #[test]
    fn deferred_physical_cut_blocks_only_pre_cut_leader_wire_occurrences() {
        let directory = TempDir::new().expect("temporary physical-cut runtime directory");
        let (mut runtime, context, keys) =
            authenticated_network_runtime(&directory, RuntimeQueueConfig::new(8, 2, 2));
        let message = signed_runtime_proposal(&context, &keys, 0x5A);
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            unreachable!("signed runtime proposal fixture carries Proposal")
        };
        let semantic_origin = context.roster
            [usize::try_from(proposal.proposer).expect("small fixture proposer")]
        .validator
        .clone();
        let (_owner_directory, _owner_ingress, mut ownerships) = preowned_leader_wire_ownerships(
            &context,
            &[(message.clone(), semantic_origin)],
            runtime.ingress.lifecycle_ordinals.clone(),
        );
        let pre_cut_fair = ownerships
            .pop()
            .expect("one productive leader-wire ownership carrier");
        let predecessor_ordinal = pre_cut_fair
            .runtime_lifecycle_ordinal()
            .expect("leader-wire carrier has an immutable logical ordinal");
        let target_cut = pre_cut_fair
            .runtime_physical_cut()
            .expect("checked dequeue freezes the target predecessor cut");
        assert!(
            u128::from(
                pre_cut_fair
                    .physical_admission_ordinal()
                    .expect("leader-wire carrier has a physical occurrence")
            ) < target_cut
        );

        let target_owner = runtime
            .mint_fresh_lifecycle_owner(
                runtime.round_tag(),
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"already-admitted deferred continuation",
            )
            .expect("mint target lifecycle after the leader-wire predecessor");
        assert!(predecessor_ordinal < target_owner.lifecycle_ordinal());
        let target = deferred_lifecycle_ownership_for_test(
            target_owner.clone(),
            7,
            RuntimeDispatchIngress::LocalOrCausal,
            None,
            target_cut,
        )
        .expect("freeze the target physical cut exactly once");
        assert!(matches!(
            deferred_lifecycle_ownership_for_test(
                target_owner.clone(),
                7,
                RuntimeDispatchIngress::LocalOrCausal,
                Some(u64::try_from(target_cut).expect("small target cut")),
                target_cut,
            ),
            Err(EnqueueError::FailClosed)
        ));
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(7, target.clone())
                .is_none()
        );
        let foreign_source = DeferredAdmissionOrdinalSource::new(7);
        let mut foreign_target = target.clone();
        foreign_target.runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
            &foreign_source,
            foreign_target.owner.causal_origin().lifecycle_key.clone(),
            foreign_target.owner.lifecycle_ordinal(),
            false,
            None,
            foreign_target.physical_cut,
        );
        assert!(
            foreign_target.validate_exact(),
            "the foreign capability can be internally self-consistent"
        );
        assert!(
            !foreign_target.validate_active_against_ingress(
                None,
                runtime.driver.deferred_admission_ordinal_source(),
            ),
            "a same-number capability minted by another source cannot own this runtime"
        );

        let make_command = |runtime: &SerializedV2Runtime<SumeragiV2Adapter>,
                            fair: FairV2IngressOwnershipEvidence| {
            let ownership = RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, fair)
                .expect("project exact leader-wire ownership into runtime");
            let authenticated = runtime
                .driver
                .authenticate(message.clone())
                .expect("authenticate the exact leader-wire proposal");
            TaggedCommand::with_ingress_ownership(
                runtime.round_tag(),
                CommandClass::Normal,
                AdapterCommand::Authenticated(authenticated),
                Instant::now(),
                ownership,
            )
        };

        let pre_cut_command = make_command(&runtime, pre_cut_fair.clone());
        runtime
            .ingress
            .enqueue(pre_cut_command)
            .expect("enqueue the real pre-cut predecessor");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("pre-cut minimum is exact"),
            Some(predecessor_ordinal),
            "a physical predecessor with an older logical identity still blocks"
        );

        runtime.ingress.commands.clear();
        let mut post_cut_fair = pre_cut_fair;
        let post_cut_ordinal =
            u64::try_from(target_cut).expect("small receiver-local physical cut");
        post_cut_fair.first.physical_admission_ordinal = post_cut_ordinal;
        post_cut_fair.latest.physical_admission_ordinal = post_cut_ordinal;
        post_cut_fair.runtime_physical_cut = target_cut.checked_add(1);
        assert!(
            post_cut_fair.validate_exact(),
            "the replay retains its exact logical identity at a fresh physical occurrence"
        );
        let post_cut_command = make_command(&runtime, post_cut_fair);
        runtime
            .ingress
            .enqueue(post_cut_command)
            .expect("enqueue the exact post-cut replay");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "a post-cut replay cannot resurrect its obsolete logical queue position"
        );

        let replay_owner = runtime
            .ingress
            .commands
            .front()
            .expect("post-cut replay remains physically queued")
            .lifecycle_owner()
            .expect("post-cut replay retains its old logical owner");
        let replay_ingress = runtime
            .ingress
            .commands
            .front()
            .and_then(|queued| queued.ingress_ownership.clone())
            .expect("post-cut replay retains its exact ingress carrier");
        runtime.ingress.commands.clear();
        let causal_completion = TaggedCommand::with_causal_origin(
            runtime.round_tag(),
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(proposal.subject),
            Instant::now(),
            replay_owner.causal_origin().clone(),
            replay_owner.lifecycle_ordinal(),
        )
        .expect("construct a local completion inheriting the replay root");
        runtime
            .ingress
            .enqueue(causal_completion)
            .expect("enqueue the post-cut causal completion");
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut causal FIFO minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "dropping the current envelope cannot drop the causal root's physical position"
        );
        runtime.ingress.commands.clear();
        runtime.pending_effect_ownership = Some(vec![RuntimeEffectOwnership::inherited(
            replay_owner.clone(),
        )]);
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("post-cut effect minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "post-cut effect and external work cannot reclaim the root's old logical rank"
        );
        runtime.pending_effect_ownership = None;
        let replay = deferred_lifecycle_ownership_for_test(
            replay_owner,
            8,
            RuntimeDispatchIngress::DirectAuthenticated,
            Some(post_cut_ordinal),
            target_cut
                .checked_add(1)
                .expect("small target cut has a successor"),
        )
        .expect("post-cut replay can cross into a distinct Busy-deferred owner");
        assert!(
            runtime
                .deferred_lifecycle_ownership
                .insert(8, replay)
                .is_none()
        );
        assert!(
            runtime
                .deferred_ingress_ownership
                .insert(8, replay_ingress)
                .is_none()
        );
        assert_eq!(
            runtime
                .minimum_active_lifecycle_ordinal_for_deferred(&target)
                .expect("deferred post-cut minimum is exact"),
            Some(target_owner.lifecycle_ordinal()),
            "crossing Busy cannot turn the post-cut replay into a predecessor"
        );
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("pairwise deferred cut relation is exact"),
            BTreeSet::from([7]),
            "the earlier target remains the sole runner-eligible continuation"
        );

        // Pairwise target-relative precedence can form a cycle even though
        // every source/cut pair is individually exact: B logically precedes
        // A, C logically precedes B, and A physically precedes C.  The global
        // selector must first exclude C as post-A-cut, then choose B by
        // logical rank.  Retiring each selected owner yields B, A, C without
        // a lasso or an empty eligible set.
        runtime.deferred_ingress_ownership.clear();
        runtime.deferred_lifecycle_ownership.clear();
        let (a, b, c) = {
            let source = runtime.driver.deferred_admission_ordinal_source();
            let make_owner = |semantic_identity: &[u8],
                              source_physical_ordinal: Option<u64>,
                              physical_cut: u128,
                              lifecycle_ordinal: u128| {
                let mut origin = RuntimeCandidateCausalOrigin::mint_fresh_root(
                    runtime.round_tag(),
                    CommandClass::Progress,
                    RuntimeFreshRootKind::StartupRecovery,
                    semantic_identity,
                );
                if let Some(source_physical_ordinal) = source_physical_ordinal {
                    origin.root_ingress_identity = Some(Hash::new(semantic_identity));
                    origin.root_ingress_physical_ownership =
                        Some(RuntimeIngressPhysicalOwnership {
                            source_ordinal: source_physical_ordinal,
                            physical_cut,
                        });
                    origin.lifecycle_key = runtime_candidate_causal_origin_lifecycle_key(&origin);
                }
                let owner = RuntimeLifecycleOwner::new(origin, lifecycle_ordinal)
                    .expect("cycle fixture owns an exact logical lifecycle");
                let runtime_seal = DeferredRuntimeOwnershipSeal::for_source_test(
                    source,
                    owner.causal_origin().lifecycle_key.clone(),
                    owner.lifecycle_ordinal(),
                    false,
                    source_physical_ordinal,
                    physical_cut,
                );
                let admission_ordinal = runtime_seal.admission_ordinal();
                let ownership = RuntimeDeferredLifecycleOwnership::new(
                    owner,
                    admission_ordinal,
                    RuntimeDispatchIngress::LocalOrCausal,
                    source_physical_ordinal,
                    physical_cut,
                    runtime_seal,
                )
                .expect("cycle fixture retains an exact source-bound runtime seal");
                assert!(ownership.validate_active_against_ingress(None, source));
                (admission_ordinal, ownership)
            };
            (
                make_owner(b"cycle-a", None, 5, 3),
                make_owner(b"cycle-b", Some(4), 9, 2),
                make_owner(b"cycle-c", Some(8), 12, 1),
            )
        };
        for (ordinal, ownership) in [a.clone(), b.clone(), c.clone()] {
            assert!(
                runtime
                    .deferred_lifecycle_ownership
                    .insert(ordinal, ownership)
                    .is_none()
            );
        }
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("two-stage selector breaks the physical/logical cycle"),
            BTreeSet::from([b.0])
        );
        assert!(runtime.deferred_lifecycle_ownership.remove(&b.0).is_some());
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("A becomes eligible after B retires"),
            BTreeSet::from([a.0])
        );
        assert!(runtime.deferred_lifecycle_ownership.remove(&a.0).is_some());
        assert_eq!(
            runtime
                .eligible_deferred_admission_ordinals()
                .expect("C becomes eligible only after its physical predecessor retires"),
            BTreeSet::from([c.0])
        );
    }

    #[test]
    fn global_lifecycle_minimum_blocks_later_fifo_until_its_completion_arrives() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let older = runtime
            .mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"older external exact request",
            )
            .expect("mint the older externally retained lifecycle");
        runtime
            .configure_external_lifecycle_owner_capacity(4)
            .expect("install the independent asynchronous bound");
        runtime
            .set_external_lifecycle_owners(vec![older.clone()])
            .expect("publish the older external owner");
        enqueue_fake(
            &mut runtime,
            owner_tag,
            CommandClass::Normal,
            FakeCommand::record(9),
        )
        .expect("enqueue later unrelated work");

        assert!(matches!(runtime.step(start), Ok(RuntimeStep::Idle)));
        let idle = runtime
            .take_last_scheduler_ownership()
            .expect("blocked scheduling still publishes exact Idle evidence");
        assert_eq!(idle.selected, RuntimeSelectedOwnerKind::Idle);
        assert!(!idle.fifo_ready);
        assert_eq!(runtime.queued_commands(), 1);

        let due = start + Duration::from_secs(10);
        assert!(matches!(runtime.step(due), Ok(RuntimeStep::Idle)));
        runtime
            .take_last_scheduler_ownership()
            .expect("blocked due clocks publish exact Idle evidence");
        assert!(runtime.timeout_owner.is_some());
        assert!(
            runtime.retransmit_owner.is_none(),
            "an absolute timeout suppresses replenishing the periodic owner until the timeout drains"
        );
        assert!(runtime.driver.timeouts.is_empty());
        assert!(runtime.driver.retransmits.is_empty());

        let older_effect = RuntimeEffectOwnership::fresh(
            older.clone(),
            RuntimeFreshRootKind::HistoricalLockedRetransmit,
        );
        runtime
            .enqueue_with_lifecycle_owner(
                owner_tag,
                CommandClass::Completion,
                FakeCommand::record(1),
                &older_effect,
            )
            .expect("enqueue the exact older completion");
        assert!(matches!(
            runtime.step(due),
            Ok(RuntimeStep::Advanced(ref effects)) if effects.len() == 1
        ));
        let selected = runtime
            .take_last_scheduler_ownership()
            .expect("completion selection publishes exact ownership");
        let RuntimeSelectedCandidateOwnership::Exact(candidate) = selected.candidate else {
            panic!("older completion must be the exact FIFO candidate");
        };
        assert_eq!(candidate.fifo_position, 1);
        assert_eq!(candidate.lifecycle_ordinal, older.lifecycle_ordinal());
        runtime
            .take_effect_ownership(1)
            .expect("test executor consumes the completion effect owner");
        assert_eq!(runtime.driver.delivered, vec![(owner_tag, 1)]);
        assert_eq!(runtime.queued_commands(), 1);

        runtime
            .set_external_lifecycle_owners(Vec::new())
            .expect("the asynchronous owner retires after its exact completion handoff");
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the older queued FIFO command now drains");
        assert_eq!(
            runtime.driver.delivered,
            vec![(owner_tag, 1), (owner_tag, 9)]
        );
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the frozen timeout drains after all older lifecycles");
        assert_eq!(runtime.driver.timeouts, vec![owner_tag]);
        assert!(runtime.timeout_owner.is_none());
        runtime
            .step_and_take_scheduler_ownership_for_test(due)
            .expect("the later frozen retransmission drains next");
        assert_eq!(runtime.driver.retransmits, vec![owner_tag]);
        assert!(runtime.retransmit_owner.is_none());
    }

    #[test]
    fn external_owner_bound_uses_effect_capacity_not_small_ingress_capacity() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let mut runtime = runtime(
            FakeDriver::new(owner_tag),
            start,
            RuntimeQueueConfig::new(8, 2, 2),
        );
        let pending_bound = 1_024usize;
        runtime
            .configure_external_lifecycle_owner_capacity(pending_bound)
            .expect("configure the executor's independent pending-work bound");
        let exact_capacity = pending_bound + MAX_EFFECTS_PER_STEP;
        let owners = (0..exact_capacity)
            .map(|ordinal| {
                let ordinal = u128::try_from(ordinal).expect("small test owner ordinal");
                let semantic = ordinal.to_le_bytes();
                RuntimeLifecycleOwner::new(
                    RuntimeCandidateCausalOrigin::mint_fresh_root(
                        owner_tag,
                        CommandClass::Progress,
                        RuntimeFreshRootKind::HistoricalLockedRetransmit,
                        &semantic,
                    ),
                    ordinal,
                )
                .expect("synthetic external owner binds its first ordinal")
            })
            .collect::<Vec<_>>();
        runtime
            .set_external_lifecycle_owners(owners)
            .expect("1024 pending owners plus one retained batch fit despite ingress capacity 8");
        assert_eq!(runtime.external_lifecycle_owners.len(), exact_capacity);
        assert!(!runtime.fail_closed);
    }

    #[test]
    fn restart_and_periodic_historical_retries_reuse_one_lifecycle_owner() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let historical = FakeEffect::historical(0xA5);
        let (mut runtime, startup) = SerializedV2Runtime::with_driver(
            FakeDriver::new(owner_tag),
            start,
            Duration::from_secs(10),
            RuntimeQueueConfig::new(8, 2, 2),
            vec![historical],
        )
        .expect("construct deterministic restart ownership");
        assert_eq!(startup, vec![historical]);
        let startup_owner = runtime
            .take_effect_ownership(1)
            .expect("consume startup ownership")
            .pop()
            .expect("one startup owner");
        assert_eq!(
            startup_owner.causality(),
            RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::StartupRecovery)
        );
        runtime
            .arm_live_clocks(start)
            .expect("startup dispatch completes before clocks arm");
        runtime.driver.timer_effects.push_back(vec![historical]);
        runtime.driver.timer_effects.push_back(vec![historical]);

        let mut retry_owners = Vec::new();
        for elapsed in [2, 4] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("periodic historical retry dispatches")
            else {
                panic!("periodic historical retry must advance");
            };
            assert_eq!(effects, vec![historical]);
            runtime
                .take_last_scheduler_ownership()
                .expect("periodic retry publishes scheduler ownership");
            retry_owners.push(
                runtime
                    .take_effect_ownership(1)
                    .expect("consume retry ownership")
                    .pop()
                    .expect("one retry owner"),
            );
        }
        assert!(retry_owners.iter().all(|ownership| {
            ownership.causality()
                == RuntimeEffectCausality::Fresh(RuntimeFreshRootKind::HistoricalLockedRetransmit)
                && ownership.owner() == startup_owner.owner()
        }));
        let cache_after_owned_retries = runtime.dormant_fresh_lifecycle_owners.len();
        assert_ne!(cache_after_owned_retries, 0);
        for elapsed in [6, 8] {
            let RuntimeStep::Advanced(effects) = runtime
                .step(start + Duration::from_secs(elapsed))
                .expect("drained historical lifecycle still services its periodic clock")
            else {
                panic!("the periodic clock must advance even after exact work drains")
            };
            assert!(
                effects.is_empty(),
                "a drained exact historical request cannot recreate physical work"
            );
            runtime
                .take_last_scheduler_ownership()
                .expect("proofless periodic stutter retains scheduler ownership");
            assert_eq!(runtime.take_effect_ownership(0), Ok(Vec::new()));
            assert_eq!(runtime.queued_commands(), 0);
            assert_eq!(
                runtime.dormant_fresh_lifecycle_owners.len(),
                cache_after_owned_retries,
                "fresh periodic episodes replace one bounded cache slot rather than growing it"
            );
        }
        assert_eq!(runtime.driver.retransmits, vec![owner_tag; 4]);

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(
                start + Duration::from_secs(9),
                &[FakeEffect::enter_view(next_tag)],
            )
            .expect("test EnterView retains positional producer ownership");
        assert!(
            runtime.dormant_fresh_lifecycle_owners.is_empty(),
            "certified view transition purges every prior-view dormant alias"
        );
    }

    #[test]
    fn dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_view() {
        let start = Instant::now();
        let owner_tag = tag(0);
        let queue = RuntimeQueueConfig::new(8, 2, 2);
        let exact_capacity = queue.capacity + MAX_EFFECTS_PER_STEP;
        let mut runtime = runtime(FakeDriver::new(owner_tag), start, queue);
        let mut last_ordinal = None;
        for identity in 0..exact_capacity {
            let identity = u128::try_from(identity)
                .expect("small dormant-cache fixture")
                .to_le_bytes();
            let owner = runtime
                .mint_fresh_lifecycle_owner(
                    owner_tag,
                    CommandClass::Progress,
                    RuntimeFreshRootKind::HistoricalLockedRetransmit,
                    &identity,
                )
                .expect("derived dormant-cache capacity admits every configured owner");
            last_ordinal = Some(owner.lifecycle_ordinal());
        }
        assert_eq!(runtime.dormant_fresh_lifecycle_owners.len(), exact_capacity);
        assert_eq!(
            runtime.mint_fresh_lifecycle_owner(
                owner_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"one owner beyond the derived bound",
            ),
            Err(EnqueueError::Full)
        );

        let next_tag = tag(1);
        runtime
            .observe_effects_with_test_ownership(start, &[FakeEffect::enter_view(next_tag)])
            .expect("test EnterView retains positional producer ownership");
        assert!(runtime.dormant_fresh_lifecycle_owners.is_empty());
        let successor = runtime
            .mint_fresh_lifecycle_owner(
                next_tag,
                CommandClass::Progress,
                RuntimeFreshRootKind::HistoricalLockedRetransmit,
                b"successor-view exact request",
            )
            .expect("view reclamation reopens the same derived cache geometry");
        assert!(
            successor.lifecycle_ordinal() > last_ordinal.expect("cache was filled"),
            "cache reclamation cannot reuse an old admission ordinal"
        );
    }
