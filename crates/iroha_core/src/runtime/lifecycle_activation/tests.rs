//! Governed change-set activation and read-only provenance; fixture scheduling is not governance qualification.

use super::*;
use crate::{
    state::{StorageReadOnly, World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_keys},
};
use iroha_data_model::runtime::lifecycle::{
    ChangeSetRecordV1, RuntimeNoticeV1, SupportAttestationV1, committee_digest,
};

/// Supply an approved record only through the test setup boundary. Its admission,
/// validator signatures, activation readiness and subsequent blocks use production code.
fn schedule(chain: &CertifiedTestChain, attest: bool) -> ChangeSetId {
    let view = chain.state().view();
    let change =
        prepare_lifecycle_change_set(view.world(), chain.network_id(), 3, [0x91; 32]).unwrap();
    let notice = RuntimeNoticeV1 {
        blocks: 1,
        ms: 0,
        approval_cushion_ms: 0,
    };
    let id = super::super::change_set::admit(
        &change,
        &chain.network_id(),
        chain.height(),
        notice,
        view.world().runtime_selection(),
        &std::collections::BTreeMap::new(),
        &super::super::change_set::PublishedDefinitions(view.world().runtime_definitions()),
    )
    .unwrap();
    drop(view);
    let committee: Vec<_> = chain
        .validators()
        .iter()
        .map(|(peer, _)| peer.public_key().clone())
        .collect();
    let mut record = ChangeSetRecordV1 {
        change_set: change.clone(),
        status: ChangeSetStatusV1::Scheduled,
        proposed_ms: 0,
        approved_height: 1,
        approved_ms: 0,
        notice,
        attestations: Vec::new(),
    };
    if attest {
        for key in fixture_keys() {
            let signed = SupportAttestationV1 {
                network: change.network,
                root: change.root,
                change_set: id,
                predecessor: change.predecessor,
                successor: change.successor,
                committee: committee_digest(&committee).unwrap(),
                tested_checkpoint: 1,
                valid_from: 1,
                valid_until: 4,
                release: [0x92; 32],
                validator: key.public_key().clone(),
            }
            .sign(key.private_key())
            .unwrap();
            super::super::change_set::record_attestation(&mut record, signed);
        }
    }
    chain.setup_world_at(1_001, |tx| {
        tx.world.runtime_change_sets.insert(id, record);
    });
    id
}

#[test]
fn governed_lifecycle_boundary_and_fingerprint_follow_committed_activation() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let before = chain
        .state()
        .view()
        .lifecycle_consensus_config_fingerprint()
        .unwrap();
    let id = schedule(&chain, true);
    // A proposed or scheduled activation is not a committed rollout boundary.
    let policy = chain.state().view().lifecycle_activation_policy().unwrap();
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
    assert_eq!(
        chain
            .state()
            .view()
            .lifecycle_consensus_config_fingerprint()
            .unwrap(),
        before
    );
    chain.commit_at(1_002, vec![]);
    assert!(
        !chain
            .state()
            .view()
            .lifecycle_activation_policy()
            .unwrap()
            .active
    );
    assert!(
        chain
            .committed(2)
            .block()
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_none())
    );
    chain.commit_at(1_003, vec![]);
    let policy = chain.state().view().lifecycle_activation_policy().unwrap();
    assert!(policy.active);
    assert_eq!(
        policy.activation,
        Some((id, std::num::NonZeroU64::new(3).unwrap()))
    );
    assert!(
        chain
            .committed(3)
            .block()
            .execution_outputs()
            .iter()
            .all(|row| row.trigger_lifecycle().is_some())
    );
    let selected = chain
        .state()
        .view()
        .lifecycle_consensus_config_fingerprint()
        .unwrap();
    assert_ne!(before, selected);
    chain.commit_at(1_004, vec![]);
    assert_eq!(
        chain.state().view().lifecycle_activation_policy().unwrap(),
        policy
    );
    assert_eq!(
        chain
            .state()
            .view()
            .lifecycle_consensus_config_fingerprint()
            .unwrap(),
        selected
    );
}

#[test]
fn missing_unanimous_support_never_selects_lifecycle_boundary() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let id = schedule(&chain, false);
    chain.commit_at(1_002, vec![]);
    chain.commit_at(1_003, vec![]);
    let view = chain.state().view();
    assert_eq!(
        view.world().runtime_change_sets().get(&id).unwrap().status,
        ChangeSetStatusV1::Lapsed(3)
    );
    let policy = view.lifecycle_activation_policy().unwrap();
    assert!(!policy.active);
    assert_eq!(policy.activation, None);
}

#[test]
fn selected_lifecycle_without_original_activated_record_fails_closed() {
    for mutation in 0..3 {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let id = schedule(&chain, true);
        chain.commit_at(1_002, vec![]);
        chain.commit_at(1_003, vec![]);
        chain.setup_world_at(1_004, |tx| {
            let mut original = tx.world.runtime_change_sets.get(&id).unwrap().clone();
            match mutation {
                0 => {
                    tx.world.runtime_change_sets.remove(id);
                }
                1 => {
                    original.status = ChangeSetStatusV1::Lapsed(3);
                    tx.world.runtime_change_sets.insert(id, original);
                }
                _ => {
                    original.status = ChangeSetStatusV1::Activated(2);
                    tx.world.runtime_change_sets.insert(id, original);
                }
            }
        });
        assert!(chain.state().view().lifecycle_activation_policy().is_err());
        assert!(
            chain
                .state()
                .view()
                .lifecycle_consensus_config_fingerprint()
                .is_err()
        );
    }
}

#[test]
fn fingerprint_binds_genesis_selection_and_original_boundary() {
    let genesis = iroha_crypto::Hash::new(b"original signed genesis");
    let mut policy = CommittedLifecyclePolicy {
        selection: release_catalog().unwrap().lifecycle_id,
        active: true,
        activation: Some((ChangeSetId([1; 32]), std::num::NonZeroU64::new(3).unwrap())),
    };
    let original = configuration_fingerprint(genesis, &policy);
    assert_ne!(
        original,
        configuration_fingerprint(iroha_crypto::Hash::new(b"other genesis"), &policy)
    );
    policy.activation.as_mut().unwrap().0 = ChangeSetId([2; 32]);
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
    policy.activation.as_mut().unwrap().0 = ChangeSetId([1; 32]);
    policy.activation.as_mut().unwrap().1 = std::num::NonZeroU64::new(4).unwrap();
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
    policy.selection = release_catalog().unwrap().baseline_id;
    assert_ne!(original, configuration_fingerprint(genesis, &policy));
}
