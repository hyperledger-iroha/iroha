fn assert_autoscale_rejects_runtime_elastic_range_corruption(scale_in: bool) {
    let mut malformed =
        autoscale_elastic_lane_config(LaneId::new(2), DataSpaceId::UNIVERSAL, 1);
    malformed.alias = "malformed-elastic-lane".to_owned();
    let other_dataspace = DataSpaceId::new(9);
    let cases = [
        (
            "manual-in-range",
            LaneConfig {
                id: LaneId::new(2),
                alias: "manual-elastic-range".to_owned(),
                ..LaneConfig::default()
            },
            None,
        ),
        ("malformed-managed", malformed, None),
        (
            "off-default-managed",
            autoscale_elastic_lane_config(LaneId::new(2), other_dataspace, 1),
            Some(other_dataspace),
        ),
        (
            "out-of-range-managed",
            autoscale_elastic_lane_config(
                LaneId::new(if scale_in { 3 } else { 4 }),
                DataSpaceId::UNIVERSAL,
                1,
            ),
            None,
        ),
    ];

    for (name, lane, extra_dataspace) in cases {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), Arc::clone(&kura), query_handle);
        state
            .set_nexus(autoscale_transition_test_nexus(
                vec![LaneConfig::default()],
                1,
                if scale_in { 3 } else { 4 },
                if scale_in { 200 } else { 100 },
            ))
            .unwrap_or_else(|err| panic!("{name}: apply autoscale test nexus config: {err}"));
        if scale_in {
            state
                .apply_lane_lifecycle_with_options(
                    &iroha_data_model::nexus::LaneLifecyclePlan {
                        additions: vec![autoscale_elastic_lane_config(
                            LaneId::new(1),
                            DataSpaceId::UNIVERSAL,
                            1,
                        )],
                        retire: Vec::new(),
                    },
                    false,
                    true,
                )
                .unwrap_or_else(|err| {
                    panic!("{name}: seed internally managed elastic lane: {err}")
                });
        }
        {
            let mut nexus = state.nexus.write();
            if let Some(dataspace) = extra_dataspace {
                nexus.dataspace_catalog = dataspace_catalog_with_extra(dataspace);
            }
            let mut lanes = nexus.lane_catalog.lanes().to_vec();
            lanes.push(lane.clone());
            let lane_count = lanes
                .iter()
                .map(|lane| lane.id.as_u32())
                .max()
                .expect("corruption test catalog is non-empty")
                .saturating_add(1);
            nexus.lane_catalog = LaneCatalog::new(
                NonZeroU32::new(lane_count).expect("nonzero lane count"),
                lanes,
            )
            .unwrap_or_else(|err| panic!("{name}: corrupted test catalog: {err}"));
            nexus.lane_config = RuntimeLaneConfig::from_catalog(&nexus.lane_catalog);
        }

        let first = autoscale_signed_block_with_committed_fragments(None, 100, 0);
        let second = autoscale_signed_block_with_committed_fragments(Some(&first), 200, 0);
        store_committed_autoscale_history_block_for_test(&state, &kura, &first);

        let mut state_block = state.block(second.header());
        if !scale_in {
            state_block.add_committed_fragments(100);
        }
        let committed_second = ValidBlock::new_unverified_for_tests(second)
            .commit_unchecked()
            .unpack(|_| {});
        state_block.maybe_apply_nexus_autoscale(&committed_second);

        let nexus = state_block.nexus.clone();
        let actual_lane_ids = nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.id)
            .collect::<BTreeSet<_>>();
        let mut expected_lane_ids = BTreeSet::from([LaneId::SINGLE, lane.id]);
        if scale_in {
            expected_lane_ids.insert(LaneId::new(1));
        }
        assert_eq!(
            actual_lane_ids,
            expected_lane_ids,
            "{}",
            if scale_in {
                format!(
                    "{name} corruption must not let cold scale-in retire a healthy managed lane"
                )
            } else {
                format!("{name} corruption must not let hot scale-out add another elastic lane")
            }
        );
        assert_eq!(
            nexus.autoscale.last_transition_height,
            0,
            "{name} corruption must not record a {} transition",
            if scale_in { "scale-in" } else { "scale-out" }
        );
    }
}

#[test]
fn autoscale_transition_runtime_elastic_range_corruption_matrix() {
    for scale_in in [false, true] {
        assert_autoscale_rejects_runtime_elastic_range_corruption(scale_in);
    }
}
