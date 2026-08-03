#[test]
fn roster_origin_relay_completion_has_authenticated_source_count_and_byte_owner() {
    const FORGED_OCCURRENCES: usize = 32;
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(20);
    let validators = validator_peers(4);
    let authenticated_non_validator_via = validator_peers(5)
        .pop()
        .expect("authenticated non-validator via fixture");
    ingress.close();
    ingress
        .configure_roster(validators.clone())
        .expect("four validator owners, one authenticated relay, and anonymous fit");
    ingress.open().expect("open configured roster");

    let mut accepted = 0_usize;
    for index in 0..FORGED_OCCURRENCES {
        let origin = &validators[index % validators.len()];
        let inbound = InboundBlockMessage::from_transport(
            v2_auxiliary_prepare(u64::try_from(index).expect("fixture index fits u64")),
            origin.clone(),
            authenticated_non_validator_via.clone(),
        );
        match handle.try_incoming_block_message_owned(inbound) {
            super::SumeragiIngressDisposition::Accepted => accepted += 1,
            super::SumeragiIngressDisposition::Retry(retained) => {
                assert_eq!(retained.sender(), Some(origin));
                assert_eq!(retained.via(), Some(&authenticated_non_validator_via));
            }
            disposition => panic!("unexpected forged-origin disposition: {disposition:?}"),
        }
    }
    assert_eq!(
        accepted, 1,
        "semantic roster identities must not multiply the authenticated hop's reserved completion owner"
    );
    {
        let state = ingress.state.lock();
        let nonempty = state
            .lanes
            .iter()
            .filter(|(_, lane)| !lane.entries.is_empty())
            .map(|(source, _)| source.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            nonempty,
            vec![super::FairV2IngressSource::Authenticated(
                authenticated_non_validator_via.clone()
            )]
        );
        assert_eq!(
            state.ready,
            std::collections::VecDeque::from([nonempty[0].clone()])
        );
        assert!(validators.iter().all(|validator| {
            state
                .lanes
                .get(&super::FairV2IngressSource::Validator(validator.clone()))
                .is_some_and(|lane| lane.entries.is_empty())
        }));
    }

    let relayed_completion = InboundBlockMessage::from_transport(
        v2_message_with_index(0),
        validators[0].clone(),
        authenticated_non_validator_via.clone(),
    );
    assert!(matches!(
        handle.try_incoming_block_message_owned(relayed_completion),
        super::SumeragiIngressDisposition::Accepted
    ));
    {
        let state = ingress.state.lock();
        assert_eq!(
            state
                .lanes
                .get(&super::FairV2IngressSource::Authenticated(
                    authenticated_non_validator_via.clone(),
                ))
                .expect("authenticated non-validator lane exists")
                .transport_completion_len,
            1
        );
        assert_eq!(
            state
                .lanes
                .get(&super::FairV2IngressSource::Authenticated(
                    authenticated_non_validator_via.clone(),
                ))
                .expect("authenticated non-validator lane exists")
                .entries
                .len(),
            2,
            "ordinary relay pressure and its reserved completion coexist"
        );
        assert!(validators.iter().all(|validator| {
            state
                .lanes
                .get(&super::FairV2IngressSource::Validator(validator.clone()))
                .is_some_and(|lane| lane.transport_completion_len == 0)
        }));
    }
    assert!(matches!(
        handle.try_incoming_block_message_owned(InboundBlockMessage::from_transport(
            v2_auxiliary_prepare(99),
            validators[1].clone(),
            authenticated_non_validator_via.clone(),
        )),
        super::SumeragiIngressDisposition::Retry(_)
    ));
    let completion = ingress
        .try_recv_if(super::fair_v2_ingress_is_transport_completion)
        .expect("trusted-relay completion bypasses ordinary relay pressure");
    assert_eq!(completion.sender(), Some(&validators[0]));
    assert_eq!(completion.via(), Some(&authenticated_non_validator_via));
    let ordinary = ingress
        .try_recv()
        .expect("the ordinary relay item remains after completion service");
    assert_eq!(ordinary.sender(), Some(&validators[0]));
    assert_eq!(ordinary.via(), Some(&authenticated_non_validator_via));

    let outsider = validator_peers(6)
        .pop()
        .expect("non-roster semantic origin fixture");
    let outsider_completion = InboundBlockMessage::from_transport(
        v2_message_with_index(1),
        outsider,
        authenticated_non_validator_via,
    );
    assert!(matches!(
        handle.try_incoming_block_message_owned(outsider_completion),
        super::SumeragiIngressDisposition::Rejected(_)
    ));
    assert!(ingress.try_recv().is_none());
}

#[test]
fn fair_v2_ingress_retains_ready_head_until_downstream_admission() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
    let validators = validator_peers(2);
    let attacker = validators[0].clone();
    let honest = validators[1].clone();
    ingress.close();
    ingress
        .configure_roster(validators)
        .expect("two validators, their progress and TimeoutVote slots, and anonymous fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(attacker.clone(), v2_message()));
    assert!(handle.try_incoming_block_message_from(honest.clone(), v2_message()));

    let mut downstream_slots = 1_usize;
    let first = ingress
        .try_recv_if(|_| downstream_slots != 0)
        .expect("attacker consumes the initially available downstream slot");
    downstream_slots -= 1;
    assert_eq!(first.sender(), Some(&attacker));
    assert_eq!(ingress.len(), 1);

    assert!(ingress.try_recv_if(|_| downstream_slots != 0).is_none());
    assert_eq!(
        ingress.len(),
        1,
        "failed downstream admission must not remove the honest head"
    );

    downstream_slots += 1;
    let retained = ingress
        .try_recv_if(|_| downstream_slots != 0)
        .expect("honest head remains available after downstream service");
    assert_eq!(retained.sender(), Some(&honest));
    assert_eq!(ingress.len(), 0);
}

#[test]
fn fair_v2_ingress_predicate_runs_outside_state_lock() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
    let validator = validator_peers(1).pop().expect("one validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("one validator and anonymous reserve fit");
    ingress.open().expect("open configured roster");
    assert!(handle.try_incoming_block_message_from(validator, v2_message()));

    let delivered = ingress
        .try_recv_if(|_| {
            assert!(
                ingress.state.try_lock().is_some(),
                "cryptographic and downstream admission work must not hold the ingress-state mutex"
            );
            true
        })
        .expect("queued message remains serviceable");
    assert!(matches!(delivered.message(), BlockMessage::V2(_)));
    assert_eq!(ingress.len(), 0);
}

#[test]
fn fair_v2_ingress_rotates_blocked_head_to_admissible_source() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(10);
    let validators = validator_peers(2);
    let blocked = validators[0].clone();
    let admissible = validators[1].clone();
    ingress.close();
    ingress
        .configure_roster(validators)
        .expect("two validators, their progress and TimeoutVote slots, and anonymous fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(blocked.clone(), v2_message()));
    assert!(handle.try_incoming_block_message_from(admissible.clone(), v2_message()));

    let selected = ingress
        .try_recv_if(|inbound| inbound.sender() == Some(&admissible))
        .expect("later admissible source bypasses a blocked ready head");
    assert_eq!(selected.sender(), Some(&admissible));
    assert_eq!(ingress.len(), 1);

    let retained = ingress
        .try_recv_if(|_| true)
        .expect("blocked source remains queued after the bypass");
    assert_eq!(retained.sender(), Some(&blocked));
    assert_eq!(ingress.len(), 0);
}

#[test]
fn fair_v2_ingress_bypasses_a_blocked_entry_within_the_same_source() {
    let (handle, ingress, _relay_receiver) = test_sumeragi_handle(8);
    let validator = validator_peers(1).pop().expect("validator fixture");
    ingress.close();
    ingress
        .configure_roster([validator.clone()])
        .expect("validator plus anonymous lane fit");
    ingress.open().expect("open configured roster");

    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(0),));
    assert!(handle.try_incoming_block_message_from(validator.clone(), v2_auxiliary_prepare(1),));
    assert!(handle.try_incoming_block_message_from(validator, v2_auxiliary_prepare(2),));

    let selected = ingress
        .try_recv_if(|inbound| vote_height(inbound) == Some(3))
        .expect("admissible later item bypasses a blocked same-source head");
    assert_eq!(vote_height(&selected), Some(3));
    assert_eq!(ingress.len(), 2);

    let first_retained = ingress
        .try_recv_if(|_| true)
        .expect("oldest blocked entry remains owned for a later fair turn");
    assert_eq!(vote_height(&first_retained), Some(1));
    let second_retained = ingress
        .try_recv_if(|_| true)
        .expect("later blocked entry retains its relative order");
    assert_eq!(vote_height(&second_retained), Some(2));
    assert_eq!(ingress.len(), 0);
}
