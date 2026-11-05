//! Platform or fixture feature coverage outside the shipping native census.

use super::*;

#[test]
fn parliament_fixture_owners_drive_real_threshold_shares_and_refuse_invalid_provider_output() {
    use crate::beacon::{
        GlobalThresholdBeaconSessionBindingV1,
        parliament_test_network_signer::{
            TestNetworkParliamentBeaconPartialSignerV1,
            deterministic_parliament_beacon_key_record_v1,
        },
        validate_global_threshold_beacon_session_v1,
        verify_finalized_global_threshold_beacon_pulse_v1,
    };

    let chain = npos_prefix(8);
    let pairs = crate::sumeragi::test_chain::fixture_keys();
    let roster = pairs
        .iter()
        .map(|pair| iroha_model_base::peer::PeerId::new(pair.public_key().clone()))
        .collect::<Vec<_>>();
    let budget = chain.state().ivm_execution_budget();
    let record = deterministic_parliament_beacon_key_record_v1(
        chain.network_id(),
        &roster,
        &pairs.iter().collect::<Vec<_>>(),
        &budget,
    )
    .unwrap();
    let binding = GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.session.network_id,
        session_id: record.session.session_id,
        roster_hash: record.session.roster_hash,
        transcript_hash: record.session.transcript_hash,
    };
    let session =
        validate_global_threshold_beacon_session_v1(&record.session, &binding, &budget).unwrap();
    let mut retained = RetainedFinalizedGlobalThresholdBeaconSessionV1 {
        session: session.clone(),
        activated_at_height: None,
        retired_at_height: None,
    };
    retained
        .activate(session.record().adaptive_dkg.finalized_at_height)
        .unwrap();
    chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_key_sessions
            .insert(binding.session_id, retained);
        transaction
            .world
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, binding.session_id);
    });
    let context = context(&chain);
    let keys = pairs
        .iter()
        .map(|pair| PublicKey::new(pair.public_key().try_to_bytes().unwrap().1.to_vec()).unwrap())
        .collect::<Vec<_>>();
    let mut producers = roster
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let signer: Option<Arc<dyn GlobalThresholdBeaconPartialSignerV1>> = if index == 2 {
                None
            } else {
                let signer = TestNetworkParliamentBeaconPartialSignerV1::try_new(
                    chain.network_id(),
                    roster.clone(),
                    peer,
                    &budget,
                )
                .unwrap();
                Some(Arc::new(if index == 3 {
                    signer.with_deliberately_invalid_share()
                } else {
                    signer
                }))
            };
            NativeBeaconProducer::new(
                chain.instance(),
                Some(keys[index].as_bytes().try_into().unwrap()),
                signer,
            )
        })
        .collect::<Vec<_>>();
    let source = chain.state().view();
    let applied = (8, context.parent_hash);
    let first = ApplicationControl {
        context,
        bytes: producers[0]
            .drive(&source, &context, applied)
            .expect("real fixture custody permits its first contribution")
            .unwrap(),
    };
    assert_eq!(
        producers[0]
            .active
            .as_ref()
            .unwrap()
            .aggregator
            .verified_partial_count(),
        1
    );
    assert!(
        producers[2]
            .drive(&source, &context, applied)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        producers[3].drive(&source, &context, applied),
        Err(NativeBeaconError::LocalSigning)
    ));
    let invalid = producers[3].active.as_ref().unwrap();
    assert!(invalid.own.is_none());
    assert_eq!(invalid.aggregator.verified_partial_count(), 0);

    let mut observer = NativeBeaconProducer::new(chain.instance(), None, None);
    assert!(
        observer
            .drive(&source, &context, applied)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        observer.accept(&source, applied, &keys[0], &first).unwrap(),
        IngressOutcome::Accepted
    );
    assert_eq!(
        observer
            .active
            .as_ref()
            .unwrap()
            .aggregator
            .verified_partial_count(),
        1
    );
    assert!(matches!(
        observer.build(&build_context(&context, 0)),
        Err(NativeBeaconError::AwaitingShares { height: 9 })
    ));
    // The negative state has one actual valid contribution, not a vacuous all-refused stall.
    assert_eq!(
        observer.accept(&source, applied, &keys[0], &first).unwrap(),
        IngressOutcome::Duplicate
    );
    let second = ApplicationControl {
        context,
        bytes: producers[1]
            .drive(&source, &context, applied)
            .unwrap()
            .unwrap(),
    };
    let mut malformed = control::decode_partial(&second.bytes).unwrap();
    malformed.signature_share[0] ^= 1;
    let bad = ApplicationControl {
        context,
        bytes: control::encode_partial(&malformed).unwrap(),
    };
    assert!(observer.accept(&source, applied, &keys[1], &bad).is_err());
    assert_eq!(
        observer
            .active
            .as_ref()
            .unwrap()
            .aggregator
            .verified_partial_count(),
        1
    );
    assert_eq!(
        observer
            .accept(&source, applied, &keys[1], &second)
            .unwrap(),
        IngressOutcome::Finalized
    );
    assert_eq!(
        observer
            .active
            .as_ref()
            .unwrap()
            .aggregator
            .verified_partial_count(),
        2
    );
    let witness = observer.build(&build_context(&context, 0)).unwrap();
    let pulse = control::decode(&witness).unwrap().unwrap();
    verify_finalized_global_threshold_beacon_pulse_v1(
        &session,
        &pulse,
        GlobalThresholdBeaconChainAnchorV1 {
            height: 8,
            block_hash: chain.committed(8).block_hash(),
        },
        &pulse_context(&context),
    )
    .unwrap();
}
