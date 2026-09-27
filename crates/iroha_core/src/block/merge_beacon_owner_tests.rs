// The certified-merge beacon owns the actual pre-State plan without a second
// effects, prune-key or roster allocation.

#[test]
fn merge_beacon_takes_original_prepared_npos_allocations() {
    let effects = NposConsensusEffects {
        penalty_actions: vec![
            iroha_data_model::consensus::NposPenaltyAction::MarkConsensusEvidenceApplied(
                iroha_data_model::consensus::NposMarkConsensusEvidenceAppliedAction {
                    evidence_key: Hash::new(b"merge beacon owner evidence"),
                    height: 1,
                },
            ),
        ],
        ..NposConsensusEffects::default()
    };
    let prune_budget = mv::allocation::AllocationBudget::new(std::mem::size_of::<Hash>());
    let mut prune_keys = mv::allocation::ChargedBuffer::new(1, &prune_budget)
        .expect("fund one merge beacon prune key");
    prune_keys
        .append(&[Hash::new(b"merge beacon owner prune")])
        .expect("append within the funded backing");
    let validator = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::BlsNormal).unwrap();
    let roster = vec![PeerId::new(validator.public_key().clone())];
    let effects_backing = effects.penalty_actions.as_ptr();
    let prune_backing = prune_keys.as_slice().as_ptr();
    assert_eq!(prune_budget.reserved_bytes(), std::mem::size_of::<Hash>());
    let roster_backing = roster.as_ptr();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 1, 0);
    let network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"merge beacon owner network")),
    );
    let parent_surface = Hash::new(b"merge beacon owner parent");
    let prepared = PreparedPristineConsensusEffects {
        header,
        effects,
        prune_keys,
        expected_anchor: None,
        roster,
    };
    let capability = prepared.into_merge_beacon(network_id, parent_surface);
    let (actual_header, actual_network, effects, prune_keys, roster, actual_parent) =
        capability.into_parts();
    assert_eq!(actual_header, header);
    assert_eq!(actual_network, network_id);
    assert_eq!(actual_parent, parent_surface);
    assert_eq!(effects.penalty_actions.as_ptr(), effects_backing);
    assert_eq!(prune_keys.as_slice().as_ptr(), prune_backing);
    assert_eq!(roster.as_ptr(), roster_backing);
    assert_eq!(effects.penalty_actions.len(), 1);
    assert_eq!(prune_keys.as_slice().len(), 1);
    assert_eq!(roster.len(), 1);
    assert_eq!(prune_budget.reserved_bytes(), std::mem::size_of::<Hash>());
    drop(prune_keys);
    assert_eq!(prune_budget.reserved_bytes(), 0);
}
