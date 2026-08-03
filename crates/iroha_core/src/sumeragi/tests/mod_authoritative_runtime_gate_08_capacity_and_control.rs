#[test]
fn fair_v2_ingress_capacity_arithmetic_overflow_fails_closed() {
    let largest_exact_roster = (usize::MAX - 2) / 4;
    assert!(super::fair_v2_ingress_required_capacity(largest_exact_roster, None).is_some());
    assert_eq!(
        super::fair_v2_ingress_required_capacity(largest_exact_roster + 1, None),
        None,
        "an unrepresentable validator-plus-relay ownership total must remain distinguishable from an exact usize::MAX capacity"
    );
    assert_eq!(
        super::fair_v2_ingress_required_byte_capacity(0, None, usize::MAX),
        Some(usize::MAX),
        "one exact usize::MAX source partition is representable"
    );
    assert_eq!(
        super::fair_v2_ingress_required_byte_capacity(1, None, usize::MAX),
        None,
        "two usize::MAX source partitions are not representable"
    );

    let exact_max = super::FairV2Ingress::new(1, usize::MAX, usize::MAX, 0, 0);
    exact_max
        .configure_roster([])
        .expect("an exact anonymous-only usize::MAX byte partition is valid");
    exact_max
        .open()
        .expect("an exact representable maximum must not be rejected as overflow");

    let validator = validator_peers(1).pop().expect("validator fixture");
    let aggregate_overflow = super::FairV2Ingress::new(6, usize::MAX, usize::MAX, 0, 0);
    let error = aggregate_overflow
        .configure_roster([validator.clone()])
        .expect_err("two source partitions must not overflow into an apparent exact fit");
    assert_eq!(error.configured(), usize::MAX);
    assert_eq!(error.required(), usize::MAX);
    assert_eq!(error.kind, super::FairV2IngressCapacityKind::Bytes);
    assert_eq!(aggregate_overflow.open(), Err(error));

    let reserve_overflow = super::FairV2Ingress::new(6, usize::MAX, usize::MAX, usize::MAX, 1);
    let error = reserve_overflow
        .configure_roster([validator])
        .expect_err("disjoint byte reserves must not overflow into an apparent exact fit");
    assert_eq!(error.configured(), usize::MAX);
    assert_eq!(error.required(), usize::MAX);
    assert_eq!(
        error.kind,
        super::FairV2IngressCapacityKind::TransportCompletionBytes
    );
    assert_eq!(reserve_overflow.open(), Err(error));
}

#[test]
fn fair_v2_ingress_rejects_timeout_vote_larger_than_its_byte_reserve() {
    let validator = validator_peers(1).pop().expect("validator fixture");
    let timeout_vote = v2_timeout_vote();
    let timeout_vote_len = encoded_v2_len(&timeout_vote);
    let reserve = timeout_vote_len.checked_sub(1).expect("non-empty envelope");
    let source_capacity = timeout_vote_len * 2;
    let ingress = super::FairV2Ingress::new(6, 2 * source_capacity, source_capacity, reserve, 0);
    ingress
        .configure_roster([validator.clone()])
        .expect("the deliberately short reserve still fits its source partition");
    ingress.open().expect("open configured roster");

    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(timeout_vote, Some(validator))),
        Err(super::FairV2IngressPushError::Rejected(_))
    ));
}

#[test]
fn fair_v2_ingress_rejects_timeout_vote_reserve_larger_than_source_partition() {
    let validator = validator_peers(1).pop().expect("validator fixture");
    let ingress = super::FairV2Ingress::new(6, 2 * 1024, 1024, 1025, 0);
    let error = ingress
        .configure_roster([validator])
        .expect_err("timeout-vote reserve must fit each validator source partition");
    assert!(error.is_bytes());
    assert_eq!(error.configured(), 1024);
    assert_eq!(error.required(), 1025);
}

#[test]
fn fair_v2_ingress_reserves_same_source_transport_completion_behind_auxiliary_pressure() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("validator plus anonymous lane fit");
    ingress.open().expect("open configured roster");

    for index in 0..3 {
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(index),)
        );
    }
    assert!(
        !handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(3),),
        "auxiliary pressure leaves the validator's transport-completion slot unconsumed"
    );
    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_message_with_index(99),));
    assert_eq!(ingress.len(), 4);

    let completion = ingress
        .try_recv_if(|inbound| payload_chunk_index(inbound) == Some(99))
        .expect("same-source transport completion bypasses the saturated auxiliary prefix");
    assert_eq!(completion.sender(), Some(&validator));
    assert_eq!(payload_chunk_index(&completion), Some(99));
    assert!(handle.try_incoming_block_message_from(validator, v2_message_with_index(100),));
    assert_eq!(
        ingress.len(),
        4,
        "service restores the exact per-validator transport-completion reservation"
    );
}

#[test]
fn fair_v2_ingress_prepare_vote_cannot_consume_commit_progress_reservation() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("validator plus anonymous lane fit");
    ingress.open().expect("open configured roster");

    let prepare =
        InboundBlockMessage::new(v2_vote(wire::GlobalPhase::Prepare), Some(validator.clone()));
    let commit =
        InboundBlockMessage::new(v2_vote(wire::GlobalPhase::Commit), Some(validator.clone()));
    assert_eq!(
        FairV2IngressClass::classify(&prepare),
        FairV2IngressClass::Auxiliary
    );
    assert_eq!(
        FairV2IngressClass::classify(&commit),
        FairV2IngressClass::Progress
    );
    let timeout = InboundBlockMessage::new(v2_timeout_vote(), Some(validator.clone()));
    assert_eq!(
        FairV2IngressClass::classify(&timeout),
        FairV2IngressClass::Progress,
        "TimeoutVote must use the per-validator protected timeout corridor"
    );
    let body_request = InboundBlockMessage::new(
        v2_certified_body_request(&validator),
        Some(validator.clone()),
    );
    assert_eq!(
        FairV2IngressClass::classify(&body_request),
        FairV2IngressClass::Progress,
        "certified body recovery must share the protected progress slot"
    );
    let commit_request = InboundBlockMessage::new(
        v2_commit_certificate_request(0, &validator),
        Some(validator.clone()),
    );
    assert_eq!(
        FairV2IngressClass::classify(&commit_request),
        FairV2IngressClass::Progress,
        "Commit-certificate recovery must share the protected progress slot"
    );

    assert!(
        handle.try_incoming_block_message_from(
            validator.clone(),
            v2_vote(wire::GlobalPhase::Prepare),
        )
    );
    for index in 0..2 {
        assert!(
            handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(index),)
        );
    }
    assert!(
        !handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(2),),
        "Prepare and auxiliary work must leave one same-source Commit slot"
    );
    assert!(handle.try_incoming_block_message_from(
            validator.clone(),
            v2_vote(wire::GlobalPhase::Commit),
        ));

    let delivered = ingress
        .try_recv_if(|inbound| vote_phase(inbound) == Some(wire::GlobalPhase::Commit))
        .expect("Commit vote bypasses the saturated auxiliary prefix");
    assert_eq!(delivered.sender(), Some(&validator));
    assert_eq!(vote_phase(&delivered), Some(wire::GlobalPhase::Commit));
}

#[test]
fn fair_v2_ingress_minimum_capacity_admits_timeout_votes() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(6);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("one validator, its progress and TimeoutVote slots, and anonymous fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(0),));
    assert!(handle.try_incoming_block_message_from(
        validator.clone(),
        v2_commit_certificate_request(0, &validator),
    ));

    let timeout = InboundBlockMessage::new(v2_timeout_vote(), Some(validator.clone()));
    assert_eq!(
        FairV2IngressClass::classify(&timeout),
        FairV2IngressClass::Progress
    );
    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_timeout_vote()));
    let delivered = ingress
        .try_recv_if(super::fair_v2_ingress_is_timeout_vote)
        .expect("minimum capacity must reserve TimeoutVote behind Prepare and recovery");
    assert_eq!(delivered.sender(), Some(&validator));
    assert!(matches!(
        delivered.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    ));
}

#[test]
fn fair_v2_ingress_newer_timeout_certificate_cannot_bypass_live_predecessor() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("validator plus anonymous lane fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_timeout_certificate(0),));
    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_timeout_certificate(1),));
    assert_eq!(ingress.len(), 2);

    let timeout_view = |inbound: &InboundBlockMessage| match inbound.message() {
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
            ..
        }) => Some(certificate.round.view),
        _ => None,
    };
    assert!(
        ingress
            .try_recv_if(|inbound| timeout_view(inbound) == Some(1))
            .is_none(),
        "a newer same-source TC must remain behind its fair-ingress predecessor"
    );
    assert_eq!(ingress.len(), 2, "a rejected bypass cannot drain either TC");
    let predecessor = ingress
        .try_recv_if(|inbound| timeout_view(inbound) == Some(0))
        .expect("the immutable first TC crosses into the runtime FIFO first");
    assert_eq!(timeout_view(&predecessor), Some(0));
    assert_eq!(ingress.len(), 1);
    let successor = ingress
        .try_recv_if(|inbound| timeout_view(inbound) == Some(1))
        .expect("the newer TC crosses only after its predecessor owns the older position");
    assert_eq!(timeout_view(&successor), Some(1));
    assert_eq!(ingress.len(), 0);
}

#[test]
fn fair_v2_ingress_control_slot_is_exactly_source_context_height_and_kind_scoped() {
    let mut validators = validator_peers(2);
    let second = validators.pop().expect("second validator fixture");
    let first = validators.pop().expect("first validator fixture");
    let predecessor = InboundBlockMessage::new(v2_timeout_certificate(0), Some(first.clone()));
    let later_same_slot = InboundBlockMessage::new(v2_timeout_certificate(1), Some(first.clone()));
    assert!(fair_v2_ingress_same_control_slot(
        &predecessor,
        &later_same_slot
    ));

    let different_source = InboundBlockMessage::new(v2_timeout_certificate(1), Some(second));
    assert!(!fair_v2_ingress_same_control_slot(
        &predecessor,
        &different_source
    ));

    let mut different_height_message = v2_timeout_certificate(1);
    let BlockMessage::V2(wire::ConsensusMessageV2 {
        payload: wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate),
        ..
    }) = &mut different_height_message
    else {
        unreachable!("timeout-certificate fixture remains a v2 certificate");
    };
    certificate.round.height = certificate.round.height.saturating_add(1);
    let different_height = InboundBlockMessage::new(different_height_message, Some(first.clone()));
    assert!(!fair_v2_ingress_same_control_slot(
        &predecessor,
        &different_height
    ));

    let different_kind = InboundBlockMessage::new(v2_timeout_vote(), Some(first.clone()));
    assert!(!fair_v2_ingress_same_control_slot(
        &predecessor,
        &different_kind
    ));

    let non_control =
        InboundBlockMessage::new(v2_commit_certificate_request(0, &first), Some(first));
    assert!(!fair_v2_ingress_same_control_slot(
        &predecessor,
        &non_control
    ));
}

#[test]
fn fair_v2_ingress_reservation_potential_does_not_increase_on_service() {
    assert_eq!(super::fair_v2_ingress_required_capacity(0, None), Some(1));
    assert_eq!(super::fair_v2_ingress_required_capacity(1, None), Some(6));
    assert_eq!(super::fair_v2_ingress_required_capacity(4, None), Some(18));
    assert_eq!(
        super::fair_v2_ingress_required_capacity(4, Some(2)),
        Some(22),
        "four validators, two authenticated non-validator sources, and anonymous reserve exactly"
    );
    assert_eq!(
        super::fair_v2_ingress_required_byte_capacity(4, Some(2), 33),
        Some(7 * 33),
        "every configured source plus anonymous owns one byte partition"
    );

    for (source_class, reserve_anonymous_completion) in [
        (super::FairV2IngressSourceClass::Anonymous, false),
        (super::FairV2IngressSourceClass::Anonymous, true),
        (super::FairV2IngressSourceClass::Authenticated, true),
        (super::FairV2IngressSourceClass::Validator, true),
    ] {
        let is_validator = source_class == super::FairV2IngressSourceClass::Validator;
        for depth in 1_usize..=8 {
            for timeout_count in 0..=usize::from(is_validator) {
                let completion_limit = 1_usize.min(depth.saturating_sub(timeout_count));
                for completion_count in 0..=completion_limit {
                    let remaining = depth - timeout_count - completion_count;
                    for progress_count in 0..=remaining {
                        let auxiliary_count =
                            depth - timeout_count - completion_count - progress_count;
                        for removed in [
                            "Auxiliary",
                            "Progress",
                            "TimeoutVote",
                            "TransportCompletion",
                        ] {
                            if (removed == "Auxiliary" && auxiliary_count == 0)
                                || (removed == "Progress" && progress_count == 0)
                                || (removed == "TimeoutVote" && timeout_count == 0)
                                || (removed == "TransportCompletion" && completion_count == 0)
                            {
                                continue;
                            }
                            let next_progress_count =
                                progress_count - usize::from(removed == "Progress");
                            let next_timeout_count =
                                timeout_count - usize::from(removed == "TimeoutVote");
                            let next_completion_count =
                                completion_count - usize::from(removed == "TransportCompletion");
                            let before = depth
                                + super::fair_v2_ingress_lane_protected_slots(
                                    source_class,
                                    reserve_anonymous_completion,
                                    depth,
                                    progress_count != 0,
                                    timeout_count != 0,
                                    completion_count != 0,
                                );
                            let after = depth - 1
                                + super::fair_v2_ingress_lane_protected_slots(
                                    source_class,
                                    reserve_anonymous_completion,
                                    depth - 1,
                                    next_progress_count != 0,
                                    next_timeout_count != 0,
                                    next_completion_count != 0,
                                );
                            assert!(
                                after <= before,
                                "service increased potential: source_class={source_class:?}, depth={depth}, progress={progress_count}, timeout={timeout_count}, completion={completion_count}, removed={removed}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn fair_v2_ingress_saturated_peer_cannot_block_an_empty_validator_timeout() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
    let validators = validator_peers(2);
    let saturated = validators[0].clone();
    let honest = validators[1].clone();
    ingress.close();
    ingress
        .configure_roster(validators)
        .expect("two validators, their progress and TimeoutVote slots, and anonymous fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(saturated.clone(), v2_auxiliary_prepare(0),));
    assert!(handle.try_incoming_block_message_from(saturated.clone(), v2_message_with_index(1),));
    assert!(
        !handle.try_incoming_block_message_from(saturated.clone(), v2_auxiliary_prepare(2),),
        "borrowed capacity must preserve both slots needed by an empty validator lane"
    );
    assert!(
        handle.try_incoming_block_message_from(honest.clone(), v2_timeout_vote()),
        "the saturated peer must not consume the honest validator's timeout slot"
    );

    let delivered = ingress
        .try_recv_if(|inbound| inbound.sender() == Some(&honest))
        .expect("honest timeout remains serviceable despite peer saturation");
    assert!(matches!(
        delivered.message(),
        BlockMessage::V2(wire::ConsensusMessageV2 {
            payload: wire::ConsensusMessageV2Payload::TimeoutVote(_),
            ..
        })
    ));
}

#[test]
fn fair_v2_ingress_non_head_service_consumes_one_source_turn() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
    let validators = validator_peers(2);
    let first_source = validators[0].clone();
    let second_source = validators[1].clone();
    ingress.close();
    ingress
        .configure_roster(validators)
        .expect("two validators, their progress and TimeoutVote slots, and anonymous fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(first_source.clone(), v2_auxiliary_prepare(0),));
    assert!(
        handle.try_incoming_block_message_from(
            first_source.clone(),
            v2_vote(wire::GlobalPhase::Commit),
        )
    );
    assert!(
        handle.try_incoming_block_message_from(second_source.clone(), v2_auxiliary_prepare(1),)
    );

    let bypass = ingress
        .try_recv_if(|inbound| vote_phase(inbound) == Some(wire::GlobalPhase::Commit))
        .expect("the first source's later admissible entry is selected");
    assert_eq!(bypass.sender(), Some(&first_source));
    assert_eq!(vote_phase(&bypass), Some(wire::GlobalPhase::Commit));

    let next = ingress
        .try_recv_if(|_| true)
        .expect("the other ready source owns the next turn");
    assert_eq!(next.sender(), Some(&second_source));
    assert_eq!(vote_height(&next), Some(2));

    let retained = ingress
        .try_recv_if(|_| true)
        .expect("the bypassed entry remains in its original source lane");
    assert_eq!(retained.sender(), Some(&first_source));
    assert_eq!(vote_height(&retained), Some(1));
    assert_eq!(ingress.len(), 0);
}
