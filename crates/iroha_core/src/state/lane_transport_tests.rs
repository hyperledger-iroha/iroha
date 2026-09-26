// Exact native fanout custody with real authenticated State/context and real
// actor FIFO tickets. Mock attempts report custody transfer, never network delivery.

fn native_transport_packet_for_test(
    fixture: &NativeProcessFixture,
    lane: &VerifiedLaneContext,
) -> crate::sumeragi::v2_lane_instance::LaneOutbound {
    let envelope = native_driver_control_for_test(fixture, lane, 0);
    crate::sumeragi::v2_lane_instance::LaneOutbound {
        canonical_bytes: norito::encode_canonical(&envelope).unwrap(),
        envelope,
        destinations: lane.frozen().committee.clone(),
    }
}

state_test! { sync native_transport_backpressure_keeps_exact_ticket_and_serves_other_peers
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport,NativeTransportAdmission,NativeTransportProgress}};
    use iroha_p2p::network::{NetworkActorAdmissionError,NetworkActorAdmissionTicketTestFixture};
    let fixture = native_process_fixture(false,std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let local = lane.frozen().committee[0].clone();
    let peers = &lane.frozen().committee[1..];
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state),Arc::clone(&guard),local,nonzero!(1_usize));
    assert!(matches!(transport.retain(&observed,native_transport_packet_for_test(&fixture,lane)),NativeTransportAdmission::Retained));
    let repeated = native_transport_packet_for_test(&fixture,lane);
    assert!(matches!(transport.retain(&observed,repeated),NativeTransportAdmission::Retained),"an exact retransmission shares the original fanout even at capacity");
    let mut different = native_transport_packet_for_test(&fixture,lane);
    different.envelope = native_driver_control_for_test(&fixture,lane,1);
    different.canonical_bytes = norito::encode_canonical(&different.envelope).unwrap();
    let original = different.canonical_bytes.clone();
    let NativeTransportAdmission::Retry(returned) = transport.retain(&observed,different) else {panic!("a distinct control must retain source custody while full")};
    assert_eq!(returned.canonical_bytes,original);
    let mut ticket_owner = None;
    let mut original_frame = None;
    let first = transport.poll_for_test(&observed,|post,ticket| {
        assert!(ticket.is_none());assert_eq!(post.peer_id,peers[0]);
        let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {panic!("native wire")};
        original_frame = Some(Arc::clone(frame));
        let (owner,ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);
        ticket_owner = Some(owner);
        Err(NetworkActorAdmissionError::Backpressured {message:post,ticket:Some(ticket),rank:1})
    }).unwrap();
    assert_eq!(first,NativeTransportProgress::Backpressured {instance:lane.instance_id(),peer:peers[0].clone(),rank:1});
    assert!(matches!(transport.retain(&observed,native_transport_packet_for_test(&fixture,lane)),NativeTransportAdmission::Retained),"a retry cannot replace the original returned post or ticket");
    for peer in &peers[1..] {
        assert_eq!(transport.poll_for_test(&observed,|post,ticket| {
            assert_eq!(&post.peer_id,peer);assert!(ticket.is_none());Ok(())
        }).unwrap(),NativeTransportProgress::Admitted {instance:lane.instance_id(),peer:peer.clone()});
    }
    for _ in 0..16 {
        assert!(matches!(transport.retain(&observed,native_transport_packet_for_test(&fixture,lane)),NativeTransportAdmission::Retained),"retransmission rearms admitted peers once without multiplying the blocked ticket");
    }
    assert_eq!(ticket_owner.as_ref().unwrap().waiter_count(),1);
    assert_eq!(ticket_owner.as_ref().unwrap().ticket_drop_cancellations(),0);
    native_process_advance(&fixture,false);
    assert_eq!(transport.poll_for_test(&observed,|_,_|panic!("stale observation cannot attempt output")).unwrap(),NativeTransportProgress::ObservationChanged);
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let mut accepted_ticket = None;
    assert_eq!(transport.poll_for_test(&current,|post,ticket| {
        assert_eq!(post.peer_id,peers[0]);
        let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {panic!("native wire")};
        assert!(Arc::ptr_eq(frame,original_frame.as_ref().unwrap()));
        assert_eq!(ticket.as_ref().unwrap().rank(),Some(1));
        accepted_ticket = ticket;Ok(())
    }).unwrap(),NativeTransportProgress::Admitted {instance:lane.instance_id(),peer:peers[0].clone()});
    for peer in &peers[1..] {
        assert_eq!(transport.poll_for_test(&current,|post,ticket| {
            assert_eq!(&post.peer_id,peer);assert!(ticket.is_none());Ok(())
        }).unwrap(),NativeTransportProgress::Admitted {instance:lane.instance_id(),peer:peer.clone()});
    }
    assert_eq!(transport.poll_for_test(&current,|_,_|panic!("drained")).unwrap(),NativeTransportProgress::Idle);
    assert_eq!(ticket_owner.as_ref().unwrap().ticket_drop_cancellations(),0,"the test actor owns the returned exact ticket now");
    drop(transport);assert!(!guard.restart_required());drop(accepted_ticket);
}

state_test! { sync native_transport_closure_retires_only_after_authenticated_complete_set
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport,NativeTransportAdmission,NativeTransportProgress}};
    use iroha_p2p::network::{NetworkActorAdmissionError,NetworkActorAdmissionTicketTestFixture};
    let fixture = native_process_fixture(false,std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];let id = lane.instance_id();
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state),Arc::clone(&guard),lane.frozen().committee[0].clone(),nonzero!(1_usize));
    assert!(matches!(transport.retain(&observed,native_transport_packet_for_test(&fixture,lane)),NativeTransportAdmission::Retained));
    let mut owner = None;
    transport.poll_for_test(&observed,|post,_| {
        let (fixture,ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);owner = Some(fixture);
        Err(NetworkActorAdmissionError::Backpressured {message:post,ticket:Some(ticket),rank:1})
    }).unwrap();
    native_process_advance(&fixture,true);
    assert_eq!(transport.poll_for_test(&observed,|_,_|panic!("stale closure")).unwrap(),NativeTransportProgress::ObservationChanged);
    assert_eq!(owner.as_ref().unwrap().waiter_count(),1);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    assert_eq!(transport.poll_for_test(&closed,|_,_|panic!("closed instance output")).unwrap(),NativeTransportProgress::Retired {instance:id,unfinished_destinations:3});
    assert_eq!(owner.as_ref().unwrap().waiter_count(),0);
    assert_eq!(owner.as_ref().unwrap().ticket_drop_cancellations(),1);
    drop(transport);assert!(!guard.restart_required());
}

state_test! { sync native_transport_rejects_changed_source_and_fails_stop_on_actor_substitution
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport,NativeTransportAdmission}};
    use iroha_p2p::network::NetworkActorAdmissionError;
    let fixture = native_process_fixture(false,std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];let local = lane.frozen().committee[0].clone();
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state),Arc::clone(&guard),local.clone(),nonzero!(1_usize));
    for change in 0..6 {
        let mut packet = native_transport_packet_for_test(&fixture,lane);
        match change {
            0 => packet.canonical_bytes[0] ^= 1,
            1 => packet.destinations.swap(0,1),
            2 => {if let iroha_data_model::block::lane_consensus::LaneMessageV1::TimeoutVote(vote) = &mut packet.envelope.message {vote.share.signature[0] ^= 1;}packet.canonical_bytes = norito::encode_canonical(&packet.envelope).unwrap();},
            3 => {packet.envelope.version += 1;packet.canonical_bytes = norito::encode_canonical(&packet.envelope).unwrap();},
            4 => {packet.canonical_bytes.pop().unwrap();},
            5 => {packet.canonical_bytes.push(0);},
            _ => unreachable!(),
        }
        let bytes = packet.canonical_bytes.clone();
        let NativeTransportAdmission::Rejected {packet,reason} = transport.retain(&observed,packet) else {panic!("changed source must refuse before P2P")};
        assert_eq!(packet.canonical_bytes,bytes);assert!(!reason.is_empty());assert!(!guard.restart_required());
    }
    assert!(matches!(transport.retain(&observed,native_transport_packet_for_test(&fixture,lane)),NativeTransportAdmission::Retained));
    let error = transport.poll_for_test(&observed,|mut post,_| {
        post.peer_id = local.clone();
        Err(NetworkActorAdmissionError::Backpressured {message:post,ticket:None,rank:0})
    }).unwrap_err();
    assert!(error.contains("substituted"),"{error}");assert!(guard.restart_required());
}

fn native_transport_global_context_for_test(
    fixture: &NativeProcessFixture,
    observed: &VerifiedLaneContexts,
) -> crate::sumeragi::v2::VerifiedHeightContext {
    let (parent, receipt) = fixture
        .state
        .kura
        .v2_finality_artifact_with_receipt(observed.carrier_height())
        .unwrap()
        .unwrap();
    let next = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    crate::sumeragi::v2::VerifiedHeightContext::successor(
        next,
        parent.validator_set_pops.clone(),
        &parent,
        &receipt,
        &parent.validator_set_pops,
    )
    .unwrap()
}

state_test! { sync native_transport_decision_reaches_global_nonmembers_and_keeps_ticket_across_rollover
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport,NativeDecisionTransportAdmission,NativeTransportProgress}};
    use iroha_p2p::network::{NetworkActorAdmissionError,NetworkActorAdmissionTicketTestFixture};
    let fixture = native_process_fixture(false,std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let global = native_transport_global_context_for_test(&fixture,&observed);
    let recipients = global.context().roster.iter().map(|entry|entry.validator.clone()).collect::<Vec<_>>();
    assert!(recipients.iter().all(|peer|!lane.frozen().committee.contains(peer)),"actual globally certifying roster differs from the lane committee");
    let FirstLaneAdmittedInputReadV1::Ready(source) = fixture.state.first_lane_admitted_input(&observed,lane).unwrap() else {panic!("first source")};
    let LaneInputBodyPreparationV1::Ready(body) = fixture.state.prepare_lane_input_body(&observed,lane,&source).unwrap() else {panic!("complete body")};
    let decision = sign_native_group_decision_for_test(lane,&fixture.keys,&body,0,0);
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state),Arc::clone(&guard),lane.frozen().committee[0].clone(),nonzero!(1_usize));
    let mut changed = decision.clone();changed.commit_qc.shares[0].signature[0] ^= 1;
    let NativeDecisionTransportAdmission::Rejected { decision: returned, reason } = transport.retain_decision(&observed,&global,changed.clone()) else {panic!("changed Decision must be rejected before fanout")};
    assert_eq!(returned,changed);assert!(!reason.is_empty());
    assert!(matches!(transport.retain_decision(&observed,&global,decision.clone()),NativeDecisionTransportAdmission::Retained));
    assert!(matches!(transport.retain_decision(&observed,&global,decision.clone()),NativeDecisionTransportAdmission::Retained),"duplicate proof does not consume another fanout");
    let mut owner = None;
    let mut frame = None;
    transport.poll_with_global_for_test(&observed,&global,|post,ticket| {
        assert!(ticket.is_none());assert_eq!(post.peer_id,recipients[0]);
        let crate::NetworkMessage::SumeragiBlock(wire) = &post.data else {panic!("native Decision frame")};
        assert!(matches!(wire.as_message(),crate::sumeragi::message::BlockMessage::NativeLaneDecision(exact) if exact.as_ref()==&decision));
        frame = Some(Arc::clone(wire));
        let (fixture,ticket) = NetworkActorAdmissionTicketTestFixture::for_topology(&post);owner=Some(fixture);
        Err(NetworkActorAdmissionError::Backpressured{message:post,ticket:Some(ticket),rank:1})
    }).unwrap();
    native_process_advance(&fixture,false);
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let NativeDecisionTransportAdmission::Retry(returned) = transport.retain_decision(&current,&global,decision.clone()) else {panic!("stale global routing must return the original Decision for retry")};
    assert_eq!(returned,decision);
    assert_eq!(transport.poll_with_global_for_test(&current,&global,|_,_|panic!("stale global routing cannot consume original output")).unwrap(),NativeTransportProgress::AwaitingGlobalRouting{instance:lane.instance_id()});
    assert_eq!(owner.as_ref().unwrap().waiter_count(),1);
    let next_global = native_transport_global_context_for_test(&fixture,&current);
    let mut served = std::collections::BTreeSet::new();
    let mut returned_ticket = None;
    for _ in 0..recipients.len() {
        transport.poll_with_global_for_test(&current,&next_global,|post,ticket| {
            assert!(served.insert(post.peer_id.clone()));
            let crate::NetworkMessage::SumeragiBlock(wire) = &post.data else {panic!("native frame")};
            assert!(Arc::ptr_eq(wire,frame.as_ref().unwrap()));
            if post.peer_id==recipients[0] {
                assert_eq!(ticket.as_ref().unwrap().rank(),Some(1));returned_ticket=ticket;
            } else {assert!(ticket.is_none());}
            Ok(())
        }).unwrap();
    }
    assert_eq!(served,recipients.into_iter().collect());
    assert_eq!(transport.poll_with_global_for_test(&current,&next_global,|_,_|panic!("drained")).unwrap(),NativeTransportProgress::Idle);
    assert_eq!(owner.as_ref().unwrap().ticket_drop_cancellations(),0);
    drop(transport);assert!(!guard.restart_required());drop(returned_ticket);
}

state_test! { sync native_transport_production_poll_retries_real_actor_pressure_without_substitution
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport, NativeTransportAdmission, NativeTransportProgress}};
    let fixture = native_process_fixture(false, std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let local = lane.frozen().committee[0].clone();
    let peers = &lane.frozen().committee[1..];
    let (network, mut actor) = crate::IrohaNetwork::actor_admission_for_tests(
        local.clone(), peers.iter().cloned().collect(), nonzero!(1_usize));
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state), Arc::clone(&guard), local, nonzero!(1_usize));
    let packet = native_transport_packet_for_test(&fixture, lane);
    let envelope = packet.envelope.clone();
    assert!(matches!(transport.retain(&observed, packet), NativeTransportAdmission::Retained));
    assert_eq!(transport.poll(&observed, None, &network).unwrap(),
        NativeTransportProgress::Admitted {instance: lane.instance_id(), peer: peers[0].clone()});
    // The real capacity-one actor remains full: every retry carries its original
    // actor ticket, instead of allocating a new FIFO rank or losing the frame.
    for peer in [&peers[1], &peers[2], &peers[1]] {
        assert_eq!(transport.poll(&observed, None, &network).unwrap(),
            NativeTransportProgress::Backpressured {instance: lane.instance_id(), peer: peer.clone(), rank: 1});
    }
    let mut frame = None;
    let mut delivered = BTreeSet::new();
    let mut inspect = |post: &iroha_p2p::network::message::Post<crate::NetworkMessage>| {
        assert!(delivered.insert(post.peer_id.clone()), "one actor occurrence per destination");
        assert_eq!(post.priority, iroha_p2p::Priority::High);
        let crate::NetworkMessage::SumeragiBlock(wire) = &post.data else {panic!("native frame")};
        assert!(matches!(wire.as_message(), crate::sumeragi::message::BlockMessage::NativeLane(exact) if exact == &envelope));
        if let Some(original) = &frame {assert!(Arc::ptr_eq(original, wire));}
        else {frame = Some(Arc::clone(wire));}
    };
    assert_eq!(actor.drain_posts(&mut inspect), 1);
    for peer in [&peers[2], &peers[1]] {
        assert_eq!(transport.poll(&observed, None, &network).unwrap(),
            NativeTransportProgress::Admitted {instance: lane.instance_id(), peer: peer.clone()});
        assert_eq!(actor.drain_posts(&mut inspect), 1);
    }
    assert_eq!(delivered, peers.iter().cloned().collect());
    assert_eq!(transport.poll(&observed, None, &network).unwrap(), NativeTransportProgress::Idle);
    drop(transport);
    assert!(!guard.restart_required());
}

state_test! { sync native_transport_production_poll_fences_closed_actor_without_losing_fanout
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport, NativeTransportAdmission, NativeTransportProgress}};
    let fixture = native_process_fixture(false, std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let local = lane.frozen().committee[0].clone();
    let (network, actor) = crate::IrohaNetwork::actor_admission_for_tests(
        local.clone(), lane.frozen().committee[1..].iter().cloned().collect(), nonzero!(1_usize));
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state), Arc::clone(&guard), local, nonzero!(1_usize));
    assert!(matches!(transport.retain(&observed, native_transport_packet_for_test(&fixture, lane)), NativeTransportAdmission::Retained));
    drop(actor);
    assert!(transport.poll(&observed, None, &network).unwrap_err().contains("closed"));
    assert!(guard.restart_required());
    assert!(transport.poll(&observed, None, &network).unwrap_err().contains("restart"));
    native_process_advance(&fixture, true);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(transport.poll(&closed, None, &network).unwrap(),
        NativeTransportProgress::Retired {instance: lane.instance_id(), unfinished_destinations: 3},
        "actor closure retained the original failed occurrence and every unfinished destination");
}

state_test! { sync native_transport_production_decision_reaches_global_nonmembers_after_rollover
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_transport::{NativeLaneTransport, NativeDecisionTransportAdmission, NativeTransportProgress}};
    let fixture = native_process_fixture(false, std::time::Instant::now());
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let global = native_transport_global_context_for_test(&fixture, &observed);
    let recipients = global.context().roster.iter().map(|entry| entry.validator.clone()).collect::<BTreeSet<_>>();
    assert!(recipients.iter().all(|peer| !lane.frozen().committee.contains(peer)));
    let FirstLaneAdmittedInputReadV1::Ready(source) = fixture.state.first_lane_admitted_input(&observed, lane).unwrap() else {panic!("first source")};
    let LaneInputBodyPreparationV1::Ready(body) = fixture.state.prepare_lane_input_body(&observed, lane, &source).unwrap() else {panic!("body")};
    let decision = sign_native_group_decision_for_test(lane, &fixture.keys, &body, 0, 0);
    let local = lane.frozen().committee[0].clone();
    let (network, mut actor) = crate::IrohaNetwork::actor_admission_for_tests(
        local.clone(), recipients.iter().cloned().collect(), nonzero!(1_usize));
    let guard = ConsensusOutputGuard::isolated();
    let mut transport = NativeLaneTransport::new(Arc::clone(&fixture.state), Arc::clone(&guard), local, nonzero!(1_usize));
    assert!(matches!(transport.retain_decision(&observed, &global, decision.clone()), NativeDecisionTransportAdmission::Retained));
    assert_eq!(transport.poll(&observed, None, &network).unwrap(),
        NativeTransportProgress::AwaitingGlobalRouting {instance: lane.instance_id()});
    assert_eq!(actor.drain_posts(|_| panic!("missing routing cannot dispatch")), 0);
    assert!(matches!(transport.poll(&observed, Some(&global), &network).unwrap(), NativeTransportProgress::Admitted {..}));
    assert!(matches!(transport.poll(&observed, Some(&global), &network).unwrap(), NativeTransportProgress::Backpressured {rank: 1, ..}));
    native_process_advance(&fixture, false);
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(transport.poll(&observed, Some(&global), &network).unwrap(), NativeTransportProgress::ObservationChanged);
    assert_eq!(transport.poll(&current, Some(&global), &network).unwrap(),
        NativeTransportProgress::AwaitingGlobalRouting {instance: lane.instance_id()});
    let next = native_transport_global_context_for_test(&fixture, &current);
    let mut delivered = BTreeSet::new();
    let mut original = None;
    let mut inspect = |post: &iroha_p2p::network::message::Post<crate::NetworkMessage>| {
        assert!(recipients.contains(&post.peer_id));
        assert!(delivered.insert(post.peer_id.clone()));
        let crate::NetworkMessage::SumeragiBlock(frame) = &post.data else {panic!("Decision frame")};
        assert!(matches!(frame.as_message(), crate::sumeragi::message::BlockMessage::NativeLaneDecision(exact) if exact.as_ref() == &decision));
        if let Some(held) = &original {assert!(Arc::ptr_eq(held, frame));}
        else {original = Some(Arc::clone(frame));}
    };
    assert_eq!(actor.drain_posts(&mut inspect), 1);
    for _ in 1..recipients.len() {
        assert!(matches!(transport.poll(&current, Some(&next), &network).unwrap(), NativeTransportProgress::Admitted {..}));
        assert_eq!(actor.drain_posts(&mut inspect), 1);
    }
    assert_eq!(delivered, recipients);
    assert_eq!(transport.poll(&current, Some(&next), &network).unwrap(), NativeTransportProgress::Idle);
    drop(transport);
    assert!(!guard.restart_required());
}
