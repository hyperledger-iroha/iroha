//! Global availability binds signed genesis and configured chain to the global instance.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_sumeragi::preimage::{InstanceKind, instance_id};
#[test]
fn constructor_binds_authenticated_genesis_and_chain_to_the_global_instance() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let state = chain.state().clone();
    let crypto = Arc::new(BlsCrypto::new());
    let genesis = crate::sumeragi::startup::core_hash_of(chain.genesis());
    let chain_id = state.chain_id_ref().to_string();
    assert_eq!(genesis.0, *state.network_id_ref().as_bytes());
    for (genesis, chain_id) in [
        (Hash32([0xab; 32]), chain_id.as_bytes()),
        (genesis, b"another-chain".as_slice()),
    ] {
        let wrong = instance_id(&*crypto, &genesis, chain_id, InstanceKind::Global, 0);
        assert_ne!(wrong, chain.instance());
        assert!(NativeGlobalAvailability::new(state.clone(), wrong, crypto.clone()).is_err());
    }
    let provider = NativeGlobalAvailability::new(state, chain.instance(), crypto).unwrap();
    assert_eq!(provider.instance(), chain.instance());
    assert!(provider.height_config(1).unwrap().is_none());
    assert!(provider.height_config(2).unwrap().is_some());
}

#[test]
fn historical_schedule_comes_from_native_execution_ancestry_without_rechecking_local_qcs() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let expected = {
        let view = chain.state().view();
        let reader = CertifiedChain::new(&view).unwrap();
        let parent = reader.certified(2).unwrap();
        let ScheduledSlot::Ready(config) = &parent.committed().commitment().schedule.next else {
            panic!("exact parent schedule")
        };
        config.height_config().unwrap()
    };
    let provider = NativeGlobalAvailability::new(
        chain.state().clone(),
        chain.instance(),
        Arc::new(BlsCrypto::new()),
    )
    .unwrap();
    let (result, counts) =
        crate::sumeragi::certified_chain::relation_counts::measure(|| provider.height_config(3));
    assert_eq!(result.unwrap(), Some(expected));
    assert!(
        counts.qcs.is_empty(),
        "local quorum bytes are not native execution authority"
    );
    assert!(
        counts.frames.contains(&2),
        "actual authenticated parent frame must be read"
    );
}
