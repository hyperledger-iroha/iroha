// Actual Worker source ownership, native preparation and strict signed genesis controls.

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap()
}

#[test]
fn pending_execution_retains_one_source_and_publishes_once() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let kura = Arc::clone(chain.kura());
    let proposal = chain.proposal(Some(2000), Vec::new());
    let source_hash = proposal.hash();
    let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    let result = pending.result();
    let inspect = |original: super::super::super::executor::PendingExecutionView<'_, '_>| {
        original
            .state
            .verify_sumeragi_execution_witness(original.block.as_ref(), original.witness)
            .unwrap();
        (
            original.block.as_ref().hash(),
            iroha_crypto::HashOf::new(original.witness),
        )
    };
    let first = pending.inspect(inspect).unwrap();
    let second = pending.inspect(inspect).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.0, source_hash);
    assert_eq!(
        kura.blocks_count(),
        1,
        "execution has not persisted anything"
    );
    pending.prepare(Signers::Quorum).unwrap();
    pending.prepare(Signers::Quorum).unwrap();
    assert!(
        pending
            .inspect(|_| ())
            .unwrap_err()
            .contains("cannot be mutated")
    );
    assert_eq!(
        kura.blocks_count(),
        1,
        "preparation alone is not durable publication"
    );
    let published = pending.publish(Signers::Quorum).unwrap();
    let repeated = pending.publish(Signers::Quorum).unwrap();
    assert_eq!(
        published.block().encode_wire().unwrap(),
        repeated.block().encode_wire().unwrap()
    );
    assert_eq!(published.result(), result);
    assert!(pending.publish(Signers::LastThree).is_err());
    drop(pending);
    assert_eq!(kura.blocks_count(), 2);
    assert_eq!(state.view().height(), 2);
    assert_eq!(chain.committed(2).result(), result);
    assert!(!chain.take_events().unwrap().is_empty());
    assert!(
        chain.take_events().unwrap().is_empty(),
        "publication delivers each event once"
    );
}

#[test]
fn original_wire_witness_and_world_tampering_fail_before_durable_staging() {
    for mutation in 0..3 {
        let mut chain = chain();
        let state = Arc::clone(chain.state());
        let kura = Arc::clone(chain.kura());
        let parent = chain.committed(1).block().encode_wire().unwrap();
        let proposal = chain.proposal(Some(2000), Vec::new());
        let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
        let original_result = pending.result();
        pending
            .inspect(move |original| {
                original
                    .state
                    .verify_sumeragi_execution_witness(original.block.as_ref(), original.witness)
                    .unwrap();
                match mutation {
                    0 => {
                        let hash = original.block.as_ref().hash();
                        let wire = original.block.as_ref().encode_wire().unwrap();
                        let key = KeyPair::from_seed(vec![0x91; 32], Algorithm::Ed25519);
                        original
                            .block
                            .as_mut()
                            .add_signature(iroha_data_model::block::BlockSignature::new(
                                1,
                                iroha_crypto::SignatureOf::from_hash(key.private_key(), hash),
                            ))
                            .unwrap();
                        assert_eq!(original.block.as_ref().hash(), hash);
                        assert_ne!(original.block.as_ref().encode_wire().unwrap(), wire);
                    }
                    1 => {
                        let write = original
                            .witness
                            .writes
                            .first_mut()
                            .expect("genuine native witness has writes");
                        write.value.push(0xFF);
                    }
                    _ => original.state.world.sumeragi_lanes.get_mut().incarnations += 1,
                }
                assert!(
                    original
                        .state
                        .verify_sumeragi_execution_witness(
                            original.block.as_ref(),
                            original.witness
                        )
                        .is_err()
                );
            })
            .unwrap();
        assert!(pending.prepare(Signers::Quorum).is_err());
        assert!(pending.publish(Signers::Quorum).is_err());
        assert_eq!(
            pending.result(),
            original_result,
            "failure never re-executes or replaces R"
        );
        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(
            kura.get_block(
                std::num::NonZeroUsize::new(1).unwrap(),
                &state.ivm_execution_budget()
            )
            .expect("original block read attempt")
            .unwrap()
            .encode_wire()
            .unwrap(),
            parent
        );
        drop(pending);
        assert_eq!(state.view().height(), 1);
    }
}

#[test]
fn pending_execution_rejects_inexact_quorum_without_publication() {
    for signers in [Signers::BelowQuorum, Signers::All] {
        let mut chain = chain();
        let kura = Arc::clone(chain.kura());
        let proposal = chain.proposal(Some(2000), Vec::new());
        let mut pending = chain.begin_proposal(proposal, Default::default()).unwrap();
        assert!(pending.prepare(signers).is_err());
        assert!(
            pending
                .prepare(Signers::Quorum)
                .unwrap_err()
                .contains("cannot be replaced")
        );
        assert_eq!(kura.blocks_count(), 1);
    }
}

#[test]
fn discarded_unprepared_execution_leaves_original_state_unchanged() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let proposal = chain.proposal(Some(2000), Vec::new());
    let pending = chain.begin_proposal(proposal, Default::default()).unwrap();
    drop(pending);
    let next = chain.proposal(Some(2000), Vec::new());
    assert_eq!(state.view().height(), 1);
    assert_eq!(state.kura().blocks_count(), 1);
    chain.commit_proposal(next, Signers::Quorum, Default::default());
    assert_eq!(state.view().height(), 2);
}

#[test]
fn prepared_genesis_derives_then_enforces_both_signed_native_policies() {
    let prepared = CertifiedTestChain::prepare(TestChainConfig::new(World::new(), 1000)).unwrap();
    let topology = super::super::super::network_topology::Topology::new(
        prepared
            .validator_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone())),
    );
    let account = AccountId::new(prepared.genesis.public_key().clone());
    let result = crate::block::ValidBlock::validate_signed_genesis(
        prepared.genesis.block().clone(),
        &topology,
        &account,
        &TimeSource::new_system(),
        &prepared.state,
        ConsensusMode::Permissioned,
    )
    .unpack(|_| {});
    let (_, overlay) =
        result.unwrap_or_else(|(_, error)| panic!("final signed policies must execute: {error}"));
    let metadata = prepared.genesis.consensus_metadata();
    assert_eq!(
        iroha_crypto::Hash::prehashed(metadata.sumeragi_context.execution_policy_hash),
        super::super::super::staged_genesis_execution_policy_hash(&overlay).unwrap(),
    );
    assert_eq!(
        iroha_crypto::Hash::prehashed(metadata.sumeragi_context.nexus_amx_context_hash),
        super::super::super::staged_genesis_nexus_amx_context_hash(&overlay),
    );
    drop(overlay);
    assert_eq!(prepared.state.view().height(), 0);
    assert_eq!(prepared.kura.blocks_count(), 0);
    assert_eq!(
        prepared.state.view().network_id(),
        &NetworkId::from_genesis_hash(prepared.genesis.expected_hash())
    );
}

#[test]
fn changed_original_genesis_configuration_is_rejected_before_publication() {
    let mut prepared =
        CertifiedTestChain::prepare(TestChainConfig::new(World::new(), 1000)).unwrap();
    Arc::get_mut(&mut prepared.state)
        .unwrap()
        .pipeline
        .amx_group_budget_ms += 1;
    let topology = super::super::super::network_topology::Topology::new(
        prepared
            .validator_keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone())),
    );
    let error = crate::block::ValidBlock::validate_signed_genesis(
        prepared.genesis.block().clone(),
        &topology,
        &AccountId::new(prepared.genesis.public_key().clone()),
        &TimeSource::new_system(),
        &prepared.state,
        ConsensusMode::Permissioned,
    )
    .unpack(|_| {})
    .err()
    .expect("configured policy mismatch must reject")
    .1;
    assert!(
        matches!(*error, crate::block::BlockValidationError::GenesisPolicyMismatch {
        expected_execution, actual_execution, expected_nexus, actual_nexus,
    } if expected_execution != actual_execution || expected_nexus != actual_nexus)
    );
    assert_eq!(prepared.state.view().height(), 0);
    assert_eq!(prepared.kura.blocks_count(), 0);
}
