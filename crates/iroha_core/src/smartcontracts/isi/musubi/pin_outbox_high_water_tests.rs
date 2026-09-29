// Native Musubi signed pin-outbox high-water transition tests.

fn pin_outbox_advance(
    network_id: iroha_data_model::NetworkId,
    authority: AccountId,
    session_id: [u8; 32],
    expected_revision: u64,
    expected_inventory_digest: [u8; 32],
    inventory_digest: [u8; 32],
) -> AdvanceMusubiPinOutboxV1 {
    AdvanceMusubiPinOutboxV1 {
        network_id,
        pin_authority: authority,
        session_id,
        expected_revision,
        expected_inventory_digest,
        inventory_digest,
    }
}

#[test]
fn pin_outbox_high_water_requires_signed_owner_network_and_contiguous_predecessor() {
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let owner = account(0xa1);
    let stranger = account(0xa2);
    let network = *state.network_id_ref();
    let header = iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::new(2).expect("nonzero height"),
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let first = pin_outbox_advance(network, owner.clone(), [0xb1; 32], 0, [0; 32], [0xc1; 32]);
    let tx_hash = HashOf::from_untyped_unchecked(Hash::new([0xd1; 32]));
    {
        let mut tx = block.transaction();
        assert!(first.clone().execute(&owner, &mut tx).is_err());
        tx.current_tx_hash = Some(tx_hash);
        assert!(first.clone().execute(&stranger, &mut tx).is_err());
        let mut wrong_network = first.clone();
        wrong_network.network_id = iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::from_untyped_unchecked(Hash::new([0xe1; 32])),
        );
        assert!(wrong_network.execute(&owner, &mut tx).is_err());
        assert!(tx.world.musubi_pin_outbox_high_waters.get(&owner).is_none());
        first
            .clone()
            .execute(&owner, &mut tx)
            .expect("initial advance");
        let record = tx
            .world
            .musubi_pin_outbox_high_waters
            .get(&owner)
            .expect("recorded first high-water");
        assert_eq!(record.revision, 1);
        assert_eq!(record.transaction_hash, *tx_hash.as_ref());
        assert!(first.clone().execute(&owner, &mut tx).is_err());
        let stale = pin_outbox_advance(
            network,
            owner.clone(),
            [0xb1; 32],
            1,
            [0xee; 32],
            [0xc2; 32],
        );
        assert!(stale.execute(&owner, &mut tx).is_err());
        let switched_session = pin_outbox_advance(
            network,
            owner.clone(),
            [0xb2; 32],
            1,
            [0xc1; 32],
            [0xc2; 32],
        );
        assert!(switched_session.execute(&owner, &mut tx).is_err());
        let second = pin_outbox_advance(
            network,
            owner.clone(),
            [0xb1; 32],
            1,
            [0xc1; 32],
            [0xc2; 32],
        );
        tx.current_tx_hash = Some(HashOf::from_untyped_unchecked(Hash::new([0xd2; 32])));
        second.execute(&owner, &mut tx).expect("contiguous advance");
        assert_eq!(
            tx.world
                .musubi_pin_outbox_high_waters
                .get(&owner)
                .expect("successor")
                .revision,
            2
        );
        tx.apply();
    }
    assert_eq!(
        block
            .world
            .musubi_pin_outbox_high_waters
            .get(&owner)
            .expect("applied high-water")
            .inventory_digest,
        [0xc2; 32]
    );
    block
        .commit_world_overlay_for_testing()
        .expect("commit pin-outbox high-water world overlay");
    assert_eq!(
        state
            .view()
            .world
            .musubi_pin_outbox_high_waters
            .get(&owner)
            .expect("committed high-water")
            .revision,
        2
    );
}
