//! Real restored-State replay and driver startup from body-independent checkpoint history.

use super::*;
use crate::sumeragi::{
    availability_schedule::AvailabilitySchedule,
    certified_chain::CertifiedChain,
    driver::traits::{Frame, SendOutcome},
    runtime_availability::NativeGlobalAvailability,
};
use iroha_data_model::sumeragi_finality::ScheduledSlot;

struct UnroutedTransport;
impl Net for UnroutedTransport {
    fn send(&self, _to: &PublicKey, _frame: &Frame) -> SendOutcome {
        SendOutcome::Rejected
    }
}

#[test]
fn portable_resume_restored_state_replays_original_successor_and_reexports_checkpoint() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use crate::sumeragi::driver::traits::BlockStore as _;
            let fixture = crate::snapshot::tests::transferred_checkpoint_fixture();
            let instance = fixture.source.instance();
            let crypto = Arc::new(BlsCrypto::new());
            let schedule = Arc::new(
                NativeGlobalAvailability::new(fixture.restored.clone(), instance, crypto.clone())
                    .unwrap(),
            );
            let expected = match &fixture.source.committed(2).commitment().schedule.next {
                ScheduledSlot::Ready(config) => config.height_config().unwrap(),
                _ => panic!("original H2 selected H3 authority"),
            };
            assert_eq!(schedule.height_config(3).unwrap(), Some(expected.clone()));
            {
                let view = fixture.restored.view();
                let original = fixture.source.committed(2);
                let incumbent = &original.commitment().schedule.current;
                assert_eq!(
                    crate::state::validator_committee::current_authority(&view).unwrap(),
                    (incumbent.generation(), incumbent.authorization),
                );
                let reader = CertifiedChain::new(&view).unwrap();
                assert_eq!(reader.instance(), instance);
                assert!(reader.committed(2).is_err());
            }
            let store = crate::sumeragi::block_store::KuraBlockStore::new(
                fixture.kura.clone(),
                crypto,
                1,
                fixture.source.root_scope(),
                crate::sumeragi::block_store::Staging::new(),
                fixture.restored.ivm_execution_budget(),
                schedule.clone(),
            );
            assert!(store.committed_body(2).is_err());
            assert_eq!(
                store.startup_headers(2, 2).unwrap().unwrap(),
                vec![fixture.source.committed(2).header().unwrap().clone()],
            );
            assert!(
                store.entry(2).is_err(),
                "header history conveys no full native metadata entry"
            );
            let (body, qc) = store
                .committed_body(3)
                .unwrap()
                .expect("actual native successor custody");
            assert_eq!(
                body.header().hash(&BlsCrypto::new()),
                fixture.source.committed(3).core_hash()
            );
            assert_eq!(qc.result, fixture.source.committed(3).result());
            drop(body);
            let (events, _) = tokio::sync::broadcast::channel(64);
            let prepared = crate::sumeragi::node::prepare(crate::sumeragi::node::PrepareInputs {
                state: fixture.restored.clone(),
                events,
                genesis: None,
                genesis_account: fixture.source.genesis_account().clone(),
                consensus_mode: iroha_data_model::parameter::system::ConsensusMode::Permissioned,
            })
            .expect("native checkpoint resume replays actual H3");
            assert_eq!(prepared.instance(), instance);
            assert_eq!(fixture.restored.committed_height(), 3);
            assert_eq!(
                fixture.restored.latest_block_hash_fast(),
                Some(fixture.source.committed(3).block_hash())
            );
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_hash(&fixture.restored).unwrap(),
                crate::snapshot::canonical_state_snapshot_hash(fixture.source.state()).unwrap(),
            );
            assert_eq!(schedule.height_config(3).unwrap(), Some(expected));
            let genesis = fixture.source.committed(1);
            let history = crate::sumeragi::driver::assemble_startup_history(
                &store,
                1,
                (genesis.core_hash(), genesis.result()),
                20,
            )
            .unwrap();
            assert_eq!(history.tip.height, 3);
            assert_eq!(
                history.recent_headers,
                (2..=3)
                    .map(|height| fixture.source.committed(height).header().unwrap().clone())
                    .collect::<Vec<_>>()
            );
            {
                let view = fixture.restored.view();
                let reader = CertifiedChain::new(&view).unwrap();
                assert_eq!(
                    reader.certified(3).unwrap().committed().block_hash(),
                    fixture.source.committed(3).block_hash()
                );
                assert!(reader.committed(2).is_err());
                assert!(reader.certified(2).is_err());
            }
            let mut config = fixture.kura_config.clone();
            config.store_dir = iroha_config_base::WithOrigin::inline(
                fixture.directory.path().join("second-transfer-kura"),
            );
            let mut exporter = crate::sumeragi::checkpoint_prefix::CheckpointPrefixExport::new(
                fixture.restored.clone(),
                2,
            )
            .unwrap();
            let sealed = exporter
                .write_kura(&config)
                .expect("re-export retains original portable prefix DATA");
            assert_eq!(
                sealed.checkpoint().header().hash(),
                fixture.source.committed(2).block_hash()
            );
            assert_eq!(
                sealed.successor_core_hash(),
                fixture.source.committed(3).core_hash()
            );
            // A genuinely unused observer key can truthfully assert first installation.
            // Its local transport rejects all routes; no simulated consensus branch runs.
            let key = KeyPair::from_seed(vec![0xfa; 32], iroha_crypto::Algorithm::BlsNormal);
            assert!(
                fixture
                    .source
                    .committed(3)
                    .commitment()
                    .schedule
                    .current
                    .committee
                    .iter()
                    .all(|member| member.validator.public_key() != key.public_key())
            );
            let (_, clock) = iroha_primitives::time::TimeSource::new_mock(Duration::ZERO);
            let node = prepared
                .start(StartInputs {
                    net: Arc::new(UnroutedTransport),
                    queue: Arc::new(Queue::test(
                        iroha_config::parameters::actual::Queue::default(),
                        &clock,
                    )),
                    key_pair: key,
                    beacon_signer: None,
                    config: NodeConfig {
                        records_dir: fixture.directory.path().join("observer-records"),
                        installation_log: fixture.directory.path().join("observer-installation"),
                        bodies_dir: fixture.directory.path().join("observer-bodies"),
                        local: SumeragiLocalOverrides::default(),
                        assert_fresh_key: true,
                        retired_keys: Vec::new(),
                    },
                    observer: Arc::new(super::super::driver::traits::NoObserver),
                    driver: DriverConfig::default(),
                })
                .expect("production driver initializes its demotion window from sealed history");
            assert_eq!(node.instance, instance);
            let recovery = node.handle().startup_recovery();
            assert!(recovery.is_ready());
            drop(node.queue_wake);
            node.lanes.shutdown();
            node.driver.shutdown();
            assert!(
                recovery.is_ready(),
                "orderly shutdown retains authenticated replay"
            );
            assert_eq!(fixture.restored.committed_height(), 3);
        })
        .unwrap()
        .join()
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure));
}
