// Real native finality, shared reducer, BLS, and descriptor-relative WAL boundary.

fn finalized_lane_wal_fixture() -> LaneContextVerifiedFixture {
    let fixture = lane_context_verified_fixture();
    let (artifact, receipt) = stage_lane_context_fixture_finality(
        &fixture.state, &fixture.block, fixture.opening.clone(), fixture.witness.clone(),
    );
    fixture.state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    fixture
}

fn native_lane_timeout_intent_for_test(
    reducer: &mut crate::sumeragi::v2_core::Reducer,
    verified: &VerifiedLaneContext,
    signer: u32,
) -> (crate::sumeragi::v2_core::Effect, crate::sumeragi::v2_lane_wire::LaneWalEnvelopeV1) {
    use crate::sumeragi::{v2_core as core, v2_lane_wire::{LaneWalEnvelopeV1, LaneWalRecordV1}};
    use iroha_data_model::block::lane_consensus::{LANE_MESSAGE_VERSION_V1, LaneRoundV1, LaneTimeoutBodyV1};
    let outcome = reducer.step(core::Event::TimeoutElapsed { tag: reducer.current_tag() }).unwrap();
    assert_eq!(outcome.effects().len(), 1, "timeout before a body must issue one durable intent");
    let effect = outcome.into_effects().pop().unwrap();
    let core::Effect::Persist { entry, .. } = &effect else { panic!("signing must wait for actual fsync") };
    let core::WalRecord::TimeoutIntent(timeout) = entry.record() else { panic!("expected timeout intent") };
    assert!(timeout.highest_prepare().is_none());
    let envelope = LaneWalEnvelopeV1 {
        version: LANE_MESSAGE_VERSION_V1, persistence_id: entry.id().get(),
        record: LaneWalRecordV1::TimeoutIntent {
            body: LaneTimeoutBodyV1 {
                round: LaneRoundV1 {
                    instance_id: Hash::prehashed(*verified.reducer_context().id().as_bytes()),
                    lane_height: verified.frozen().next_lane_height,
                    voting_view: timeout.round().view(),
                },
                highest_prepare: None,
            }, signer,
        },
    };
    (effect, envelope)
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_wal_prepayload_failover_replays_fsync_before_ack
    use crate::sumeragi::{v2_core as core, v2_lane_wal::LaneSafetyWal,
        v2_lane_wire::{LaneAuthenticator, LaneWalEnvelopeV1, LaneWalRecordV1}};
    use iroha_data_model::block::lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneMessageV1, LaneSignatureShareV1, LaneTcV1, LaneTimeoutVoteV1,
    };
    let fixture = finalized_lane_wal_fixture();
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let verified = &current.contexts()[0];
    let context = verified.reducer_context();
    let silent = context.leader(0);
    let survivors = context.roster().iter().enumerate().filter_map(|(index, validator)|
        (validator.id() != silent).then_some(index as u32)).collect::<Vec<_>>();
    assert_eq!(survivors.len(), 3);
    let mut owners = Vec::new();
    let mut timeouts = Vec::new();
    for (position, signer) in survivors.into_iter().enumerate() {
        let mut wal = LaneSafetyWal::open(&fixture.state.kura, verified, signer).unwrap();
        let mut reducer = wal.recover(core::Generation::new(0)).unwrap();
        let resumed = reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap();
        assert!(resumed.effects().is_empty());
        let (effect, native) = native_lane_timeout_intent_for_test(&mut reducer, verified, signer);
        let mut substituted = native.clone(); substituted.persistence_id += 1;
        assert!(wal.append_issued(&effect, &substituted).is_err());
        let ack = wal.append_issued(&effect, &native).unwrap();
        let signed_effects = if position == 0 {
            // Actual file is closed after fsync, before its ack reaches the reducer.
            drop(reducer); drop(wal); drop(ack);
            wal = LaneSafetyWal::open(&fixture.state.kura, verified, signer).unwrap();
            reducer = wal.recover(core::Generation::new(0)).unwrap();
            assert!(reducer.durable_state().timeout_intent(core::Round::new(context.height(), 0)).is_some());
            reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap().into_effects()
        } else {
            reducer.step(ack).unwrap().into_effects()
        };
        let [core::Effect::Sign { tag, message: core::SignableMessage::TimeoutVote(intent) }] = signed_effects.as_slice()
            else { panic!("exact durable timeout must resume signing: {signed_effects:?}") };
        let LaneWalRecordV1::TimeoutIntent { body, .. } = native.record else { unreachable!() };
        let key = fixture.validators.iter().find(|key| key.public_key() == verified.frozen().committee[signer as usize].public_key()).unwrap();
        let signature = {
            let _publication = fixture.state.consensus_publication_lease();
            assert!(current.is_current(&fixture.state));
            Signature::try_new(key.private_key(), &body.signature_preimage().unwrap()).unwrap().payload().to_vec()
        };
        let vote = LaneTimeoutVoteV1 { body, share: LaneSignatureShareV1 { signer, signature: signature.clone() } };
        let core::Event::TimeoutVoteReceived { vote: authenticated, .. } = LaneAuthenticator::new(verified)
            .event(&LaneMessageV1::TimeoutVote(vote.clone()), *tag).unwrap() else { unreachable!() };
        assert_eq!(authenticated.vote(), *intent);
        let outbound = reducer.step(core::Event::Signed { tag: *tag, signature: core::OpaqueSignature::new(signature) }).unwrap();
        assert!(matches!(outbound.effects(), [core::Effect::Broadcast(core::ConsensusMessageV2::TimeoutVote(_))]));
        timeouts.push(vote);
        owners.push((signer, wal, reducer));
    }
    timeouts.sort_by_key(|vote| vote.share.signer);
    let tc = LaneTcV1 { round: timeouts[0].body.round, votes: timeouts.clone() };
    let mut recovered_broadcast = None;
    for (signer, mut wal, mut reducer) in owners {
        let mut persisted = None;
        for vote in &timeouts {
            let event = LaneAuthenticator::new(verified).event(&LaneMessageV1::TimeoutVote(vote.clone()), reducer.current_tag()).unwrap();
            for effect in reducer.step(event).unwrap().into_effects() {
                assert!(matches!(&effect, core::Effect::Persist { entry, .. } if matches!(entry.record(), core::WalRecord::InstallTimeout(_))));
                assert!(persisted.replace(effect).is_none());
            }
        }
        let effect = persisted.expect("the three surviving native signatures must form a timeout quorum");
        let core::Effect::Persist { entry, .. } = &effect else { unreachable!() };
        let native = LaneWalEnvelopeV1 { version: LANE_MESSAGE_VERSION_V1,
            persistence_id: entry.id().get(), record: LaneWalRecordV1::InstallTimeout(tc.clone()) };
        assert_eq!(reducer.current_tag().view(), 0, "view change is not published before fsync");
        let ack = wal.append_issued(&effect, &native).unwrap();
        assert!(wal.append_issued(&effect, &native).is_err(), "a receipt cannot create a second frame");
        let outcome = reducer.step(ack).unwrap();
        let [core::Effect::EnterView { tag, certificate, protected_lock },
            core::Effect::Broadcast(core::ConsensusMessageV2::TimeoutCertificate(broadcast))] = outcome.effects()
            else { panic!("complete TC continuation must retain its dissemination effect: {:?}", outcome.effects()) };
        assert_eq!(tag.view(), 1);
        assert!(protected_lock.is_none());
        assert_eq!(certificate, broadcast);
        assert_eq!(reducer.current_tag().view(), 1);
        assert_ne!(context.leader(1), silent);
        assert!(reducer.durable_state().highest_prepare().is_none());
        assert!(reducer.durable_state().decision().is_none());
        // Lose this first broadcast and restart the complete physical owner.
        // Recovery must retain a serviceable control, not only a view number.
        let broadcast = broadcast.clone();
        drop(reducer); drop(wal);
        let wal = LaneSafetyWal::open(&fixture.state.kura, verified, signer).unwrap();
        let mut replayed = wal.recover(core::Generation::new(0)).unwrap();
        assert_eq!(replayed.current_tag().view(), 1);
        let resumed = replayed.step(core::Event::ResumeAfterReplay { tag: replayed.current_tag() }).unwrap();
        assert!(resumed.effects().is_empty());
        let retransmitted = replayed.step(core::Event::RetransmitElapsed { tag: replayed.current_tag() }).unwrap();
        assert_eq!(retransmitted.effects(), &[core::Effect::Broadcast(core::ConsensusMessageV2::TimeoutCertificate(broadcast.clone()))]);
        recovered_broadcast = Some(broadcast);
    }
    // The previously silent validator missed every vote and first broadcast.
    // A recovered TC alone advances it, without fetching a proposal or body.
    let late_signer = context.roster().iter().position(|validator| validator.id() == silent).unwrap() as u32;
    let mut late_wal = LaneSafetyWal::open(&fixture.state.kura, verified, late_signer).unwrap();
    let mut late = late_wal.recover(core::Generation::new(0)).unwrap();
    assert!(late.step(core::Event::ResumeAfterReplay { tag: late.current_tag() }).unwrap().effects().is_empty());
    let event = LaneAuthenticator::new(verified).event(&LaneMessageV1::TimeoutCertificate(tc.clone()), late.current_tag()).unwrap();
    let core::Event::TimeoutCertificateReceived { certificate, .. } = &event else { unreachable!() };
    assert_eq!(Some(certificate), recovered_broadcast.as_ref());
    let outcome = late.step(event).unwrap();
    let [effect @ core::Effect::Persist { entry, .. }] = outcome.effects() else { panic!("late TC must be persisted") };
    let native = LaneWalEnvelopeV1 { version: LANE_MESSAGE_VERSION_V1, persistence_id: entry.id().get(), record: LaneWalRecordV1::InstallTimeout(tc) };
    let ack = late_wal.append_issued(effect, &native).unwrap();
    assert!(matches!(late.step(ack).unwrap().effects(), [core::Effect::EnterView { tag, .. }] if tag.view() == 1));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_wal_rejects_foreign_key_and_corrupt_complete_frame
    use crate::sumeragi::{v2_core as core, v2_lane_wal::LaneSafetyWal};
    use std::io::{Read as _, Seek as _, Write as _};
    let fixture = finalized_lane_wal_fixture();
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let verified = &current.contexts()[0];
    assert!(LaneSafetyWal::open(&fixture.state.kura, verified, 4).is_err());
    let mut wal0 = LaneSafetyWal::open(&fixture.state.kura, verified, 0).unwrap();
    let mut wal1 = LaneSafetyWal::open(&fixture.state.kura, verified, 1).unwrap();
    let mut reducer = wal1.recover(core::Generation::new(0)).unwrap();
    reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap();
    let (effect, native) = native_lane_timeout_intent_for_test(&mut reducer, verified, 1);
    assert!(wal0.append_issued(&effect, &native).is_err());
    assert!(wal0.recover(core::Generation::new(0)).unwrap().durable_state().timeout_intent(core::Round::new(verified.frozen().next_lane_height, 0)).is_none());
    let ack = wal1.append_issued(&effect, &native).unwrap();
    assert!(matches!(ack, core::Event::Persisted { .. }));
    drop(wal0); drop(wal1);
    let mut paths = std::fs::read_dir(fixture.state.kura.sumeragi_v2_storage_root().join("wal")).unwrap()
        .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|extension| extension == "wal"))
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| std::fs::metadata(path).unwrap().len());
    assert_eq!(paths.len(), 2);
    let path = paths.last().unwrap();
    let mut file = std::fs::OpenOptions::new().read(true).write(true).open(path).unwrap();
    let mut original = Vec::new(); file.read_to_end(&mut original).unwrap();
    let mut corrupt = original.clone(); *corrupt.last_mut().unwrap() ^= 1;
    file.seek(std::io::SeekFrom::Start(0)).unwrap(); file.write_all(&corrupt).unwrap(); file.sync_all().unwrap();
    assert!(LaneSafetyWal::open(&fixture.state.kura, verified, 1).is_err(), "complete frame corruption cannot be treated as a crash tail");
    file.seek(std::io::SeekFrom::Start(0)).unwrap(); file.write_all(&original).unwrap(); file.sync_all().unwrap(); drop(file);
    let reopened = LaneSafetyWal::open(&fixture.state.kura, verified, 1).unwrap();
    assert!(reopened.recover(core::Generation::new(0)).unwrap().durable_state().timeout_intent(core::Round::new(verified.frozen().next_lane_height, 0)).is_some());
}
