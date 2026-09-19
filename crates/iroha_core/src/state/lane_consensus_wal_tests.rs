// Real native finality, shared reducer, BLS, and descriptor-relative WAL boundary.

fn finalized_lane_wal_fixture() -> Box<LaneContextVerifiedFixture> {
    let fixture = lane_context_verified_fixture();
    let (artifact, receipt) = stage_lane_context_fixture_finality(
        &fixture.state,
        &fixture.block,
        fixture.opening.clone(),
        fixture.witness.clone(),
    );
    fixture
        .state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    fixture
}

fn native_lane_timeout_intent_for_test(
    reducer: &mut crate::sumeragi::v2_core::Reducer,
    verified: &VerifiedLaneContext,
    signer: u32,
) -> (
    crate::sumeragi::v2_core::Effect,
    crate::sumeragi::v2_lane_wire::LaneWalEnvelopeV1,
) {
    use crate::sumeragi::{
        v2_core as core,
        v2_lane_wire::{LaneWalEnvelopeV1, LaneWalRecordV1},
    };
    use iroha_data_model::block::lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneRoundV1, LaneTimeoutBodyV1,
    };
    let outcome = reducer
        .step(core::Event::TimeoutElapsed {
            tag: reducer.current_tag(),
        })
        .unwrap();
    assert_eq!(
        outcome.effects().len(),
        1,
        "timeout before a body must issue one durable intent"
    );
    let effect = outcome.into_effects().pop().unwrap();
    let core::Effect::Persist { entry, .. } = &effect else {
        panic!("signing must wait for actual fsync")
    };
    let core::WalRecord::TimeoutIntent(timeout) = entry.record() else {
        panic!("expected timeout intent")
    };
    assert!(timeout.highest_prepare().is_none());
    let envelope = LaneWalEnvelopeV1 {
        version: LANE_MESSAGE_VERSION_V1,
        persistence_id: entry.id().get(),
        record: LaneWalRecordV1::TimeoutIntent {
            body: LaneTimeoutBodyV1 {
                round: LaneRoundV1 {
                    instance_id: Hash::prehashed(*verified.reducer_context().id().as_bytes()),
                    lane_height: verified.frozen().next_lane_height,
                    voting_view: timeout.round().view(),
                },
                highest_prepare: None,
            },
            signer,
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
            let recovered = wal.recover_with_native(core::Generation::new(0)).unwrap();
            assert_eq!(recovered.native_records(), std::slice::from_ref(&native),
                "fsync-before-ack retains the exact unsigned native timeout intent");
            let (replayed, witnesses) = recovered.into_parts();
            assert_eq!(norito::encode_canonical(&witnesses[0]).unwrap(), norito::encode_canonical(&native).unwrap());
            reducer = replayed;
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
        let recovered = wal.recover_with_native(core::Generation::new(0)).unwrap();
        assert_eq!(recovered.native_records().len(), 2);
        assert_eq!(recovered.native_records()[1], native,
            "recovered TC retains every exact native timeout vote and signature");
        let LaneWalRecordV1::InstallTimeout(retained_tc) = &recovered.native_records()[1].record else { unreachable!() };
        assert_eq!(retained_tc, &tc);
        for (actual, expected) in retained_tc.votes.iter().zip(&timeouts) {
            assert_eq!(actual.body.signature_preimage().unwrap(), expected.body.signature_preimage().unwrap());
            assert_eq!(actual.share, expected.share);
        }
        let (mut replayed, _) = recovered.into_parts();
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

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_wal_recovery_retains_exact_proposal_manifest_and_signing_preimage
    use crate::sumeragi::{v2_core as core, v2_lane_wal::LaneSafetyWal,
        v2_lane_wire::{LaneAuthenticator, LaneWalEnvelopeV1, LaneWalRecordV1}};
    use iroha_data_model::block::{consensus_v2 as wire, lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneJustificationV1, LaneManifestV1, LaneMessageV1,
        LaneProposalBodyV1, LaneProposalV1, LaneRoundV1, LaneValueKindV1, LaneValueRefV1,
        lane_availability_hash,
    }};
    let fixture = finalized_lane_wal_fixture();
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let verified = &current.contexts()[0];
    let context = verified.reducer_context();
    let signer = context.roster().iter().position(|validator| validator.id() == context.leader(0)).unwrap() as u32;
    let layout = verified.frozen().da_layout;
    // The local Ready event is the unit fixture's supplied body-adapter input.
    // This test proves physical WAL/native-witness custody, not the not-yet-wired
    // production input/RS16 Ready boundary.
    let byte_len = 37;
    let chunk_count = wire::expected_encoded_chunk_count(byte_len, layout).unwrap();
    let chunk_root = Hash::new(b"proposal WAL fixture availability root");
    let value = LaneValueRefV1 {
        instance_id: Hash::from(verified.instance_id().0),
        admitted_binding_hash: verified.frozen().admitted_binding_hash,
        kind: LaneValueKindV1::Execution,
        origin_view: 0, origin_producer: signer,
        descriptor_hash: Hash::new(b"proposal WAL immutable native descriptor"),
        payload_hash: Hash::new(b"proposal WAL exact body identity"),
        availability_hash: lane_availability_hash(layout, chunk_root, byte_len, chunk_count).unwrap(),
    };
    let manifest = LaneManifestV1 { value, layout, chunk_root, byte_len, chunk_count };
    let proposal = LaneProposalBodyV1 {
        round: LaneRoundV1 { instance_id: value.instance_id, lane_height: context.height(), voting_view: 0 },
        proposer: signer, manifest, justification: LaneJustificationV1::Opening,
    };
    let preimage = proposal.signature_preimage().unwrap();
    let mut wal = LaneSafetyWal::open(&fixture.state.kura, verified, signer).unwrap();
    let mut reducer = wal.recover(core::Generation::new(0)).unwrap();
    assert!(reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap().effects().is_empty());
    let outcome = reducer.step(core::Event::LocalProposalReady {
        tag: reducer.current_tag(), manifest: core::PayloadManifest::new(
            core::Subject::new(value.subject_hash().unwrap().into()), core::Digest::new(value.payload_hash.into()),
            core::Digest::new(chunk_root.into()), byte_len, chunk_count,
        ),
    }).unwrap();
    let [effect @ core::Effect::Persist { entry, .. }] = outcome.effects() else { panic!("exact durable proposal intent before signing") };
    let native = LaneWalEnvelopeV1 { version: LANE_MESSAGE_VERSION_V1, persistence_id: entry.id().get(), record: LaneWalRecordV1::ProposalIntent(proposal.clone()) };
    let expected_entry = entry.clone();
    let mut tampered = native.clone();
    let LaneWalRecordV1::ProposalIntent(tampered_body) = &mut tampered.record else { unreachable!() };
    tampered_body.manifest.chunk_root = Hash::new(b"substituted unsigned root");
    assert!(wal.append_issued(effect, &tampered).is_err());
    assert!(wal.recover_with_native(core::Generation::new(0)).unwrap().native_records().is_empty(),
        "failed manifest authentication cannot append a physical record");
    let ack = wal.append_issued(effect, &native).unwrap();
    assert!(matches!(ack, core::Event::Persisted { .. }));
    drop(ack); drop(reducer); drop(wal);
    let wal = LaneSafetyWal::open(&fixture.state.kura, verified, signer).unwrap();
    let recovered = wal.recover_with_native(core::Generation::new(0)).unwrap();
    assert_eq!(recovered.native_records(), std::slice::from_ref(&native));
    let (mut replayed, retained) = recovered.into_parts();
    let LaneWalRecordV1::ProposalIntent(retained_proposal) = &retained[0].record else { unreachable!() };
    assert_eq!(retained_proposal.manifest, manifest);
    assert_eq!(retained_proposal.signature_preimage().unwrap(), preimage);
    assert_eq!(LaneAuthenticator::new(verified).encode_wal(&retained[0], &expected_entry).unwrap(), norito::encode_canonical(&native).unwrap());
    let before_resume = replayed.clone();
    let fenced = replayed.step(core::Event::RetransmitElapsed { tag: replayed.current_tag() }).unwrap();
    assert_eq!(fenced.disposition(), core::StepDisposition::Ignored(core::IgnoreReason::RecoveryPending),
        "retained native evidence does not bypass ResumeAfterReplay");
    assert!(fenced.effects().is_empty());
    assert_eq!(replayed, before_resume);
    let resumed = replayed.step(core::Event::ResumeAfterReplay { tag: replayed.current_tag() }).unwrap();
    let [core::Effect::Sign { tag, message: core::SignableMessage::Proposal(intent) }] = resumed.effects() else { panic!("exact replayed proposal signing effect: {:?}", resumed.effects()) };
    let key = fixture.validators.iter().find(|key| key.public_key() == verified.frozen().committee[signer as usize].public_key()).unwrap();
    let signature = {
        let _publication = fixture.state.consensus_publication_lease();
        assert!(current.is_current(&fixture.state));
        Signature::try_new(key.private_key(), &retained_proposal.signature_preimage().unwrap()).unwrap().payload().to_vec()
    };
    let event = LaneAuthenticator::new(verified).event(&LaneMessageV1::Proposal(LaneProposalV1 {
        body: retained_proposal.clone(), signature,
    }), *tag).unwrap();
    let core::Event::ProposalReceived { proposal: authenticated, .. } = event else { unreachable!() };
    assert_eq!(authenticated.proposal(), intent);
    assert_eq!(retained, vec![native], "signing does not rewrite the native recovery evidence");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_wal_recovery_rejects_physically_valid_foreign_intent_and_bad_tc
    use crate::sumeragi::{safety_wal::SafetyWal, v2_core as core, v2_lane_wal::LaneSafetyWal,
        v2_lane_wire::{LaneAuthenticator, LaneWalRecordV1}};
    use iroha_data_model::block::lane_consensus::{LaneSignatureShareV1, LaneTcV1, LaneTimeoutVoteV1};
    for corrupt_certificate in [false, true] {
        let fixture = finalized_lane_wal_fixture();
        let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
        let verified = &current.contexts()[0];
        let wal = LaneSafetyWal::open(&fixture.state.kura, verified, 0).unwrap();
        let mut reducer = wal.recover(core::Generation::new(0)).unwrap();
        reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap();
        let (_, mut native) = native_lane_timeout_intent_for_test(&mut reducer, verified, 0);
        let LaneWalRecordV1::TimeoutIntent { body, .. } = &native.record else { unreachable!() };
        if corrupt_certificate {
            let mut votes = (0..3).map(|signer| {
                let key = fixture.validators.iter().find(|key| key.public_key() == verified.frozen().committee[signer as usize].public_key()).unwrap();
                let signature = Signature::try_new(key.private_key(), &body.signature_preimage().unwrap()).unwrap().payload().to_vec();
                LaneTimeoutVoteV1 { body: body.clone(), share: LaneSignatureShareV1 { signer, signature } }
            }).collect::<Vec<_>>();
            votes[1].share.signature[0] ^= 1;
            native.record = LaneWalRecordV1::InstallTimeout(LaneTcV1 { round: body.round, votes });
        } else {
            let LaneWalRecordV1::TimeoutIntent { signer, .. } = &mut native.record else { unreachable!() };
            *signer = 1;
        }
        drop(reducer); drop(wal);
        let paths = std::fs::read_dir(fixture.state.kura.sumeragi_v2_storage_root().join("wal")).unwrap()
            .map(|entry| entry.unwrap().path()).filter(|path| path.extension().is_some_and(|extension| extension == "wal")).collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        let path = &paths[0];
        let auth = LaneAuthenticator::new(verified);
        // Deliberately inject invalid logical/native evidence through the real
        // physical writer: checksums/fsync succeed and are not the rejected boundary.
        let mut physical = SafetyWal::open_with_kura_authority(
            &fixture.state.kura, fixture.state.kura.mint_safety_wal_directory_authority().unwrap(),
            path.file_name().unwrap().to_str().unwrap(), auth.wal_identity(0).unwrap(),
        ).unwrap();
        let bytes = norito::encode_canonical(&native).unwrap();
        let receipt = physical.append(&bytes).unwrap();
        let record = physical.recovered_records().last().unwrap();
        assert!(record.exactly_matches_receipt(receipt));
        assert_eq!(record.payload(), bytes);
        if corrupt_certificate {
            assert!(auth.decode_storage_wal(record).is_err(), "native BLS verification rejects tampered TC");
            assert!(auth.decode_storage_wal_with_envelope(record).is_err());
        } else {
            assert!(auth.decode_storage_wal(record).is_ok(), "other roster member is cryptographically/structurally valid");
            assert!(auth.decode_storage_wal_with_envelope(record).is_ok());
        }
        drop(physical);
        let exact = std::fs::read(path).unwrap();
        let reopened = LaneSafetyWal::open(&fixture.state.kura, verified, 0).unwrap();
        assert!(reopened.recover_with_native(core::Generation::new(0)).is_err(),
            "no partial reducer/native evidence escapes failed cryptographic or local-key replay");
        assert!(reopened.recover(core::Generation::new(0)).is_err(), "old API shares the exact same replay kernel");
        assert_eq!(std::fs::read(path).unwrap(), exact, "recovery rejection is read-only");
    }
}
