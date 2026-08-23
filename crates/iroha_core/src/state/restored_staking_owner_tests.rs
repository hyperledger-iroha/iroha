    #[test]
    fn state_block_does_not_activate_restored_non_owner_staking_rows() {
        let kura = crate::kura::Kura::blank_kura_for_testing();
        let query = crate::query::store::LiveQueryStore::start_test();
        let mut state =
            State::new_for_testing(World::default(), std::sync::Arc::clone(&kura), query);
        let owner_lane = LaneId::SINGLE;
        let sibling_lane = LaneId::new(1);
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: sibling_lane,
                    alias: "restored-staking-sibling".to_owned(),
                    dataspace_id: DataSpaceId::UNIVERSAL,
                    visibility: LaneVisibility::Public,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("shared-dataspace lane catalog");
        let mut nexus = iroha_config::parameters::actual::Nexus {
            lane_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };
        nexus.lane_config = DerivedLaneConfig::from_catalog(&nexus.lane_catalog);
        state
            .set_nexus(nexus)
            .expect("apply shared-dataspace Nexus config");
        let owner_kp = crate::state::checked_keypair();
        let pending_sibling_kp = crate::state::checked_keypair();
        let jailed_sibling_kp = crate::state::checked_keypair();
        let owner_validator = DMAccountId::of(owner_kp.public_key().clone());
        let pending_sibling_validator = DMAccountId::of(pending_sibling_kp.public_key().clone());
        let jailed_sibling_validator = DMAccountId::of(jailed_sibling_kp.public_key().clone());
        let record =
            |lane_id, validator: DMAccountId, peer_key, status: PublicLaneValidatorStatus| {
                lane_validator_record(lane_id, &validator, PeerId::from(peer_key), 10_u32, status)
            };
        {
            let mut block = state.world.public_lane_validators.block();
            block.insert(
                (owner_lane, owner_validator.clone()),
                record(
                    owner_lane,
                    owner_validator.clone(),
                    owner_kp.public_key().clone(),
                    PublicLaneValidatorStatus::PendingActivation(3),
                ),
            );
            block.insert(
                (sibling_lane, pending_sibling_validator.clone()),
                record(
                    sibling_lane,
                    pending_sibling_validator.clone(),
                    pending_sibling_kp.public_key().clone(),
                    PublicLaneValidatorStatus::PendingActivation(3),
                ),
            );
            block.insert(
                (sibling_lane, jailed_sibling_validator.clone()),
                record(
                    sibling_lane,
                    jailed_sibling_validator.clone(),
                    jailed_sibling_kp.public_key().clone(),
                    PublicLaneValidatorStatus::Jailed("vrf_penalty_epoch_2".to_owned()),
                ),
            );
            block.commit();
        }
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(9).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut state_block = state.block(header);
        state_block.activate_due_public_lane_validators(3);
        state_block.clear_expired_vrf_public_lane_jails(3);
        let owner = state_block
            .world
            .public_lane_validators
            .get(&(owner_lane, owner_validator))
            .expect("canonical owner validator remains present");
        assert!(matches!(owner.status, PublicLaneValidatorStatus::Active));
        let pending_sibling = state_block
            .world
            .public_lane_validators
            .get(&(sibling_lane, pending_sibling_validator))
            .expect("pending non-owner validator remains present");
        assert!(matches!(
            pending_sibling.status,
            PublicLaneValidatorStatus::PendingActivation(3)
        ));
        let jailed_sibling = state_block
            .world
            .public_lane_validators
            .get(&(sibling_lane, jailed_sibling_validator))
            .expect("jailed non-owner validator remains present");
        assert!(matches!(
            jailed_sibling.status,
            PublicLaneValidatorStatus::Jailed(ref reason)
                if reason == "vrf_penalty_epoch_2"
        ));
    }
