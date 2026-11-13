// Shared carriers retain original queue ownership until replaced through the removal owner.
#[test]
fn sccp_keepalive_queue_refresh_releases_only_stale_own_custody() {
    use crate::smartcontracts::isi::sccp::{
        committee_storage::CommitteeFixtureStorage as _, test_support::sample_generation,
    };
    use iroha_data_model::{isi::chain::Keepalive, sccp::params::SccpParametersV1};
    let parameters = SccpParametersV1::taira_default();
    let generation = sample_generation(1);
    let deadline = generation.start_timestamp_ms + parameters.committee_heartbeat_ms;
    let world = world_with_test_domains();
    {
        let mut block = world.block();
        *block.sccp_parameters.get_mut() = Some(parameters);
        *block.sccp_committee_current.get_mut() = 1;
        block.sccp_committee_generations.insert(1, generation);
        block.commit();
    }
    let mut state = State::new(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    install_single_validator_topology_for_queue_test(&mut state, 0xB7);
    let (clock, time) = TimeSource::new_mock(Duration::from_millis(deadline));
    let queue = Queue::test(config_factory(), &time);
    let key = checked_random_queue_keypair();
    let authority = AccountId::new(key.public_key().clone());
    let carrier = || {
        accepted_tx_with(
            authority.clone(),
            &key,
            &time,
            vec![Keepalive {}.into()],
            Metadata::default(),
        )
    };
    let original = carrier();
    let old_hash = original.hash_as_entrypoint();
    assert!(state.view().world().accounts().get(&authority).is_none());
    queue
        .push(original, state.view())
        .expect("standalone unregistered Ed25519 carrier is fee-free");
    assert!(queue.has_any_fresh_pending_keepalive(&state.view(), deadline));
    assert_eq!(
        queue.has_fresh_pending_keepalive(&state.view(), &authority, deadline),
        Ok(true)
    );
    clock.advance(Duration::from_millis(30_002));
    let now = deadline + 30_002;
    assert!(!queue.has_any_fresh_pending_keepalive(&state.view(), now));
    assert_eq!(
        queue.has_fresh_pending_keepalive(&state.view(), &authority, now),
        Ok(false)
    );
    assert!(!queue.txs.contains_key(&old_hash));
    assert!(!queue.routing_plans.contains_key(&old_hash));
    assert_eq!(
        queue
            .pending_sccp_exempt
            .lock()
            .keepalive_holder(&authority),
        None
    );
    let replacement = carrier();
    let new_hash = replacement.hash_as_entrypoint();
    assert_ne!(
        old_hash, new_hash,
        "freshness requires a new original signature"
    );
    queue
        .push(replacement, state.view())
        .expect("stale holder's index and reservations were released");
    assert!(queue.has_any_fresh_pending_keepalive(&state.view(), now));
    let other_key = checked_random_queue_keypair();
    let other = AccountId::new(other_key.public_key().clone());
    assert_eq!(
        queue.has_fresh_pending_keepalive(&state.view(), &other, now),
        Ok(false)
    );
    assert_eq!(
        queue
            .pending_sccp_exempt
            .lock()
            .keepalive_holder(&authority),
        Some(new_hash)
    );
    assert_eq!(queue.active_len(), 1);
}

#[test]
fn obsolete_local_time_keepalive_preserves_fee_exemption_and_foreign_custody() {
    use crate::{
        smartcontracts::isi::{
            sccp::keepalive,
            triggers::{
                progress,
                specialized::{SpecializedAction, SpecializedTrigger},
            },
        },
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_data_model::{
        events::time::Schedule,
        isi::chain::Keepalive,
        prelude::{ExecutionTime, Repeats, TimeEventFilter},
    };
    let mut config = TestChainConfig::new(World::default(), 1_000);
    let mut fees = zero_rate_routing_fees();
    fees.base_fee = Quantity::from(1_u32);
    config
        .genesis_parameters
        .push(iroha_data_model::parameter::Parameter::Custom(
            fees.into_custom_parameter().unwrap(),
        ));
    let chain = CertifiedTestChain::start(config).unwrap();
    let state = chain.state();
    assert!(
        crate::smartcontracts::isi::sccp::store::parameters::get(state.view().world()).is_none()
    );
    assert_eq!(
        state
            .view()
            .world()
            .parameters()
            .root_fees()
            .unwrap()
            .base_fee,
        Quantity::from(1_u32)
    );
    let key = checked_random_queue_keypair();
    let authority = AccountId::new(key.public_key().clone());
    let other_key = checked_random_queue_keypair();
    let other = AccountId::new(other_key.public_key().clone());
    let install = |start| {
        let mut world = state.world.block();
        let mut tx = world.triggers.transaction();
        tx.remove(&"time_queue".parse().unwrap());
        let mut action = SpecializedAction::new(
            Executable::Instructions(
                vec![Log::new(Level::INFO, "time queue".to_owned()).into()].into(),
            ),
            Repeats::Exactly(1),
            authority.clone(),
            TimeEventFilter(ExecutionTime::Schedule(Schedule {
                start_ms: start,
                period_ms: None,
            })),
        )
        .unwrap();
        action.metadata.insert(
            "__registered_block_height".parse().unwrap(),
            Json::new(1_u64),
        );
        action
            .metadata
            .insert(progress::DECLARATION_KEY.parse().unwrap(), Json::new(1_u32));
        progress::initialize(&mut action.metadata, 1);
        tx.add_time_trigger(SpecializedTrigger::new(
            "time_queue".parse().unwrap(),
            action,
        ))
        .unwrap();
        tx.apply();
        world.commit();
    };
    install(2_000);
    let (clock, time) = TimeSource::new_mock(Duration::from_millis(2_000));
    let queue = Queue::test(config_factory(), &time);
    let carrier = |owner: &AccountId, signer: &KeyPair| {
        let signed = TransactionBuilder::new_with_time_source(
            chain.network_id(),
            owner.clone(),
            &time,
            iroha_data_model::transaction::FeePaymentIntent::authority(vec![], None),
        )
        .with_instructions([Keepalive {}])
        .sign(signer.private_key());
        AcceptedTransaction::accept_with_time_source(
            signed,
            &chain.network_id(),
            Duration::from_millis(10),
            TransactionParameters::default(),
            &iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::new(vec![
                iroha_crypto::Algorithm::Ed25519,
            ])
            .unwrap(),
            &time,
        )
        .unwrap()
    };
    let own = carrier(&authority, &key);
    let old_hash = own.hash_as_entrypoint();
    {
        let view = state.view();
        assert!(exemption_needs_committed_time(
            view.world(),
            own.external().unwrap().payload()
        ));
        let mut access = EagerAdmissionStateAccess::new(&view, 2, 1_000);
        // This is the actual strict queue quote. A zero-rate fixture would hide
        // the missing-parent bug by accepting a nonexempt empty fee intent.
        assert!(
            access
                .recheck_external_nexus_fee_admission(&queue, &own, Some(DataSpaceId::UNIVERSAL),)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            queue.recheck_external_nexus_fee_admission(
                &own,
                view.world(),
                &view.nexus,
                &view.pipeline,
                &view.zk.gas,
                2,
                Some(DataSpaceId::UNIVERSAL),
                None,
            ),
            Err(Error::NexusFeeAdmissionRejected {
                code: FeeRejectionCode::InvalidFeeIntent,
                ..
            })
        ));
    }
    queue.push(own, state.view()).unwrap();
    let foreign = carrier(&other, &other_key);
    let foreign_hash = foreign.hash_as_entrypoint();
    queue.push(foreign, state.view()).unwrap();
    install(2_002);
    clock.advance(Duration::from_millis(2));
    assert_eq!(
        keepalive::due(state.view().world(), Some(1_000), 2, 2_001),
        Ok(false)
    );
    assert_eq!(
        keepalive::due(state.view().world(), Some(1_000), 2, 2_003),
        Ok(true)
    );
    assert_eq!(
        queue.has_fresh_pending_keepalive(&state.view(), &authority, 2_002),
        Ok(false)
    );
    assert!(!queue.txs.contains_key(&old_hash));
    assert!(queue.txs.contains_key(&foreign_hash));
    let fresh = carrier(&authority, &key);
    queue
        .push(fresh, state.view())
        .expect("obsolete local source released its exclusive index");
    assert_eq!(
        queue.has_fresh_pending_keepalive(&state.view(), &authority, 2_002),
        Ok(true)
    );
    assert!(queue.txs.contains_key(&foreign_hash));
}
