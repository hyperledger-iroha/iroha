    #[test]
    fn restored_carrierless_leader_wires_stay_dormant_until_exact_post_capacity_replay() {
        for cut in [
            RestoredLeaderWireCut::Reserved,
            RestoredLeaderWireCut::Ingress,
            RestoredLeaderWireCut::Runtime,
            RestoredLeaderWireCut::Volatile,
        ] {
            let fixture = restored_leader_wire_fixture(cut);
            let mut occurrence = 0_u64;
            loop {
                let unrelated = InboundBlockMessage::new(
                    v2_commit_certificate_request(occurrence, &fixture.validator),
                    Some(fixture.validator.clone()),
                );
                match fixture.ingress.try_push(unrelated) {
                    Ok(super::FairV2IngressPushDisposition::Enqueued) => {
                        occurrence = occurrence
                            .checked_add(1)
                            .expect("bounded ingress fills before u64 exhaustion");
                    }
                    Err(super::FairV2IngressPushError::Full(_)) => break,
                    _ => panic!("unexpected unrelated admission for {cut:?}"),
                }
            }
            assert_ne!(
                occurrence, 0,
                "unrelated traffic must enter despite the dormant {cut:?} owner"
            );
            assert!(matches!(
                fixture.ingress.try_push(InboundBlockMessage::new(
                    fixture.message.clone(),
                    Some(fixture.validator.clone()),
                )),
                Err(super::FairV2IngressPushError::Full(_))
            ));
            {
                let state = fixture.ingress.state.lock();
                assert!(state.open, "ordinary backpressure cannot fail-stop {cut:?}");
                let record = state
                    .leader_wire_lifecycles
                    .get(&fixture.token.slot)
                    .expect("restored slot remains retained");
                assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Dormant);
                assert_eq!(record.token, fixture.token);
            }
            assert_eq!(
                fixture
                    .gate
                    .earliest_ingress_scheduler_ordinal()
                    .expect("read full replay selector"),
                None,
                "a Full exact replay cannot activate the dormant {cut:?} owner"
            );
            while fixture.ingress.try_recv_if(|_| true).is_some() {}

            let mut newer = fixture.message.clone();
            let BlockMessage::V2(envelope) = &mut newer else {
                unreachable!("leader-wire restart fixture is a v2 envelope");
            };
            let wire::ConsensusMessageV2Payload::Proposal(proposal) = &mut envelope.payload else {
                unreachable!("leader-wire restart fixture carries Proposal");
            };
            proposal.round.view = proposal
                .round
                .view
                .checked_add(1)
                .expect("fixture view has a successor");
            assert!(matches!(
                fixture.ingress.try_push(InboundBlockMessage::new(
                    newer,
                    Some(fixture.validator.clone()),
                )),
                Err(super::FairV2IngressPushError::Full(_))
            ));
            assert!(
                fixture.ingress.state.lock().open,
                "a newer conflicting identity must wait without fail-stop for {cut:?}"
            );

            assert!(
                matches!(
                    fixture.ingress.try_push(InboundBlockMessage::new(
                        v2_commit_certificate_request(occurrence, &fixture.validator),
                        Some(fixture.validator.clone()),
                    )),
                    Ok(super::FairV2IngressPushDisposition::Enqueued)
                ),
                "unrelated traffic must still bypass the dormant {cut:?} slot"
            );
            assert!(fixture.ingress.try_recv_if(|_| true).is_some());

            assert!(matches!(
                fixture.ingress.try_push(InboundBlockMessage::new(
                    fixture.message.clone(),
                    Some(fixture.validator.clone()),
                )),
                Ok(super::FairV2IngressPushDisposition::Enqueued)
            ));
            assert_eq!(
                fixture
                    .gate
                    .earliest_ingress_scheduler_ordinal()
                    .expect("read activated replay selector"),
                Some(fixture.token.scheduler_ordinal())
            );
            {
                let state = fixture.ingress.state.lock();
                let record = state
                    .leader_wire_lifecycles
                    .get(&fixture.token.slot)
                    .expect("exact retry retained its old slot");
                assert_eq!(record.status, super::FairV2IngressLeaderWireStatus::Ingress);
                assert_eq!(record.token, fixture.token);
            }

            let mut replay = fixture
                .ingress
                .try_recv_if(|_| true)
                .expect("exact replay owns a physical carrier");
            let mut ownership = replay
                .take_ingress_ownership()
                .expect("exact replay retains ingress ownership");
            assert_eq!(ownership.leader_wire_token(), Some(&fixture.token));
            fixture
                .ingress
                .bind_leader_wire_runtime_ownership(&mut ownership)
                .expect("exact replay rebinds its immutable runtime owner");
            let runtime = ownership
                .leader_wire_runtime_receipt()
                .expect("runtime receipt is installed");
            assert_eq!(runtime.token(), &fixture.token);
            if let Some(restored_owner) = fixture.runtime_owner {
                assert_eq!(runtime.owner(), restored_owner);
            }
            fixture
                .ingress
                .mark_leader_wire_volatile_terminal(runtime)
                .expect("publish replay tombstone");
            assert!(
                matches!(
                    fixture.ingress.try_push(InboundBlockMessage::new(
                        fixture.message,
                        Some(fixture.validator),
                    )),
                    Ok(super::FairV2IngressPushDisposition::Coalesced)
                ),
                "the drained {cut:?} lifecycle cannot resurrect its old ingress stage"
            );
            assert!(fixture.ingress.try_recv_if(|_| true).is_none());
        }
    }

