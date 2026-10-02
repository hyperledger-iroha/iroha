// Restore the original current and predecessor global lane state, never old frozen contexts.

fn actual_lane_snapshot_chain() -> (crate::sumeragi::test_chain::CertifiedTestChain, KeyPair) {
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig, fixture_validators};
    use iroha_data_model::{
        parameter::{Parameter, system::SumeragiParameters},
        sumeragi_lanes::{SumeragiFixedLane, SumeragiLaneMember, SumeragiLanePolicy},
    };
    let mut config = TestChainConfig::new(World::new(), 1000);
    let authority = config.genesis_key.clone();
    config.world.account_permissions.insert(
        AccountId::new(authority.public_key().clone()),
        BTreeSet::from([Permission::from(
            iroha_executor_data_model::permission::parameter::CanSetParameters,
        )]),
    );
    config.genesis_parameters.push(Parameter::Custom(
        SumeragiLanePolicy {
            da_layout: iroha_sumeragi::availability::recommended_data_availability_layout(),
            anchor_freshness: 4,
            max_merge_blocks: 8,
            stall_window: 1,
            lane_params: SumeragiParameters::default(),
            fixed: vec![SumeragiFixedLane {
                lane: LaneId::new(2),
                dataspace: DataSpaceId::new(0),
                committee: fixture_validators()
                    .into_iter()
                    .map(|(peer, pop)| SumeragiLaneMember { peer, pop })
                    .collect(),
            }],
            routes: Vec::new(),
            autoscale: None,
        }
        .into_custom_parameter(),
    ));
    (
        CertifiedTestChain::start(config).expect("actual certified lane genesis"),
        authority,
    )
}

// A component projection can validate both real lane cuts without installing a
// native State tip. Certified replay independently establishes that authority.
fn validated_lane_snapshot_projection(
    snapshot: &norito::json::Value,
    state: &State,
) -> Result<Cell<iroha_data_model::sumeragi_lanes::SumeragiLaneState>, norito::json::Error> {
    let invalid = |cut: &str, message: String| norito::json::Error::InvalidField {
        field: format!("world.sumeragi_lanes.{cut}"),
        message,
    };
    let encoded = snapshot
        .as_object()
        .and_then(|value| value.get("world"))
        .and_then(norito::json::Value::as_object)
        .and_then(|value| value.get("sumeragi_lanes"))
        .ok_or_else(|| invalid("blocks", "missing complete lane component".to_owned()))?;
    let lanes: Cell<iroha_data_model::sumeragi_lanes::SumeragiLaneState> =
        norito::json::from_value(encoded.clone())?;
    let height = u64::try_from(state.committed_height()).expect("lane fixture height fits");
    validate_sumeragi_lane_state(state.network_id, height, lanes.view().get())
        .map_err(|message| invalid("blocks", message))?;
    let undo = lanes.predecessor_view();
    if height > 0 {
        let previous = undo.get().as_ref().ok_or_else(|| {
            invalid(
                "revert",
                "committed lane cut requires its predecessor".to_owned(),
            )
        })?;
        validate_sumeragi_lane_state(state.network_id, height - 1, previous)
            .map_err(|message| invalid("revert", message))?;
    } else if undo.get().is_some() {
        return Err(invalid(
            "revert",
            "height-zero lane state cannot retain predecessor undo".to_owned(),
        ));
    }
    drop(undo);
    Ok(lanes)
}

state_test! { sync snapshot_global_lane_state_preserves_opening_and_closure_predecessors
    let (mut chain, authority) = actual_lane_snapshot_chain();
    for closed in [false, true] {
        if closed {
            for time in [2000, 3000] { chain.commit_at(time, Vec::new()); }
            let mut policy = crate::sumeragi::lanes::lane_policy(chain.state().view().world()).expect("completed original routing metadata read").unwrap();
            policy.fixed.clear();
            let update = chain.sign(&authority, [InstructionBox::from(iroha_data_model::isi::SetParameter::new(
                iroha_data_model::parameter::Parameter::Custom(policy.into_custom_parameter())
            ))], 3999);
            assert!(chain.commit_at(4000, vec![update]).into_iter().all(|accepted| accepted));
        }
        let state = chain.state();
        let current = state.world.sumeragi_lanes.view().get().clone();
        let previous = state.world.sumeragi_lanes.predecessor_view().get().clone()
            .expect("actual lane step retains its original predecessor");
        assert_eq!(current.lanes.len(), 1);
        assert_eq!(current.lanes[0].closing.is_some(), closed);
        assert_eq!(previous.lanes.is_empty(), !closed);
        if closed { assert!(previous.lanes[0].closing.is_none()); }
        let snapshot = norito::json::to_value(&**state).unwrap();
        assert!(!snapshot.as_object().unwrap().contains_key("lane_consensus_contexts"));
        assert!(matches!(
            deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(chain.kura())),
            Err(super::deserialize::StateRestoreError::NativeExecutionReplayRequired)
        ));
        let decoded = validated_lane_snapshot_projection(&snapshot, state)
            .expect("decode both complete actual lane component cuts");
        assert_eq!(decoded.view().get(), &current);
        assert_eq!(decoded.predecessor_view().get(), &Some(previous.clone()));
        let (mut replay, _) = actual_lane_snapshot_chain();
        replay.replay_from(&chain).expect("replay exact original certified lane history");
        let restored = Arc::clone(replay.state());
        assert_eq!(restored.world.sumeragi_lanes.view().get(), &current);
        assert_eq!(restored.world.sumeragi_lanes.predecessor_view().get(), &Some(previous.clone()));
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),
            crate::snapshot::canonical_state_snapshot_hash(state).unwrap());
        let carrier = chain.committed(state.committed_height() as u64);
        {
            let reverted = restored.block_and_revert(carrier.block().header());
            assert_eq!(reverted.world.sumeragi_lanes(), &previous);
            validate_sumeragi_lane_state(restored.network_id, state.committed_height() as u64 - 1,
                reverted.world.sumeragi_lanes()).unwrap();
        }
        assert_eq!(restored.world.sumeragi_lanes.view().get(), &current);
        assert_eq!(restored.world.sumeragi_lanes.predecessor_view().get(), &Some(previous.clone()));
        for bad in [norito::json::Value::Null, norito::json::to_value(&current).unwrap()] {
            let mut changed = snapshot.clone();
            changed.as_object_mut().unwrap().get_mut("world").unwrap().as_object_mut().unwrap()
                .get_mut("sumeragi_lanes").unwrap().as_object_mut().unwrap().insert("revert".into(), bad);
            let error = validated_lane_snapshot_projection(&changed, state)
                .err().expect("a current lifecycle cut cannot replace its predecessor");
            assert!(error.to_string().contains("world.sumeragi_lanes.revert"), "{error}");
        }
        let mut obsolete = snapshot.clone();
        obsolete.as_object_mut().unwrap().insert("lane_consensus_contexts".into(), norito::json::Value::Null);
        assert!(deserialize_state_snapshot_value_with_kura(obsolete, Arc::clone(chain.kura())).is_err());
        let mut missing = snapshot;
        missing.as_object_mut().unwrap().get_mut("world").unwrap().as_object_mut().unwrap()
            .remove("sumeragi_lanes");
        assert!(deserialize_state_snapshot_value_with_kura(missing, Arc::clone(chain.kura())).is_err());
    }
}

state_test! { sync snapshot_global_lane_state_rejects_invalid_current_and_predecessor_credentials
    let (mut chain, _authority) = actual_lane_snapshot_chain();
    chain.commit_at(2000, Vec::new());
    let snapshot = norito::json::to_value(&**chain.state()).unwrap();
    for cut in ["blocks", "revert"] {
        let original = if cut == "blocks" {
            chain.state().world.sumeragi_lanes.view().get().clone()
        } else {
            chain.state().world.sumeragi_lanes.predecessor_view().get().clone().unwrap()
        };
        for mutation in 0..3 {
            let mut value = original.clone();
            match mutation {
                0 => value.lanes[0].committee[0].pop[0] ^= 1,
                1 => value.lanes[0].merged.block_hash[0] ^= 1,
                _ => value.lanes[0].created_at = 3,
            }
            let mut changed = snapshot.clone();
            changed.as_object_mut().unwrap().get_mut("world").unwrap().as_object_mut().unwrap()
                .get_mut("sumeragi_lanes").unwrap().as_object_mut().unwrap()
                .insert(cut.into(), norito::json::to_value(&value).unwrap());
            let error = validated_lane_snapshot_projection(&changed, chain.state())
                .err().expect("each actual lane cut must validate its own source credentials");
            assert!(error.to_string().contains(&format!("world.sumeragi_lanes.{cut}")), "{error}");
        }
    }
}

state_test! { sync snapshot_global_lane_state_height_zero_rejects_any_undo
    let state = blank_test_state();
    let snapshot = norito::json::to_value(&state).unwrap();
    deserialize_state_snapshot_value_with_kura(snapshot.clone(), Arc::clone(&state.kura)).unwrap();
    let mut invalid = snapshot;
    invalid.as_object_mut().unwrap().get_mut("world").unwrap().as_object_mut().unwrap()
        .get_mut("sumeragi_lanes").unwrap().as_object_mut().unwrap().insert("revert".into(),
            norito::json::to_value(state.world.sumeragi_lanes.view().get()).unwrap());
    let error = deserialize_state_snapshot_value_with_kura(invalid, Arc::clone(&state.kura))
        .err().expect("height zero has no predecessor even for empty lane state");
    assert!(error.to_string().contains("height-zero lane state"), "{error}");
}
