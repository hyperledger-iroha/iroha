// Snapshot restoration preserves active-ID projections for both action images.

#[test]
fn set_json_restores_all_active_indexes_for_latest_block_replacement() {
    use crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY;
    use std::time::Duration;

    let set = sample_set();
    let ids = ["data1", "pipe1", "time1", "call1"].map(|id| id.parse::<dm::TriggerId>().unwrap());
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        for id in &ids {
            tx.inspect_by_id_mut(id, |action| {
                action.metadata_mut().insert(
                    "__registered_block_height".parse().unwrap(),
                    Json::from(1_u64),
                );
                action
                    .metadata_mut()
                    .insert("__registered_at_ms".parse().unwrap(), Json::from(0_u64));
            })
            .unwrap();
        }
        tx.apply();
        block.commit();
    }
    let mut expected = ids.to_vec();
    expected.sort();
    assert_eq!(active_trigger_ids(&set), expected);
    let new_id: dm::TriggerId = "new_at_h".parse().unwrap();
    {
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.inspect_by_id_mut(&ids[0], |action| {
            action.metadata_mut().insert(
                TRIGGER_ENABLED_METADATA_KEY.parse().unwrap(),
                Json::from(false),
            );
        })
        .unwrap();
        assert!(tx.remove(&ids[1]));
        for id in [&ids[2], &ids[3]] {
            tx.inspect_by_id_mut(id, |action| action.set_repeats(dm::Repeats::Exactly(0)))
                .unwrap();
        }
        let action = SpecializedAction::new(
            dm::Executable::Instructions(ConstVec::from(Vec::<dm::InstructionBox>::new())),
            dm::Repeats::Exactly(1),
            checked_account_id(),
            dm::ExecuteTriggerEventFilter::new(),
        )
        .unwrap();
        tx.add_by_call_trigger(SpecializedTrigger::new(new_id.clone(), action))
            .unwrap();
        tx.apply();
        block.commit();
    }
    assert_eq!(active_trigger_ids(&set), vec![new_id.clone()]);
    let encoded = json::to_json(&set).unwrap();
    let restored: Set = json::from_json(&encoded).unwrap();
    assert_eq!(active_trigger_ids(&restored), vec![new_id]);
    {
        let replacement = restored.block_and_revert();
        let original_replacement = set.block_and_revert();
        let mut actual = replacement
            .active_trigger_ids_iter()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(actual, expected);
        assert_eq!(
            replacement.active_trigger_ids_iter().collect::<Vec<_>>(),
            original_replacement
                .active_trigger_ids_iter()
                .collect::<Vec<_>>()
        );
        let event = dm::TimeEvent {
            interval: dm::TimeInterval::new_since_to(Duration::ZERO, Duration::from_secs(2)),
        };
        assert_eq!(
            replacement
                .match_time_event(event, 6, 2_000, 16)
                .collect::<Vec<_>>(),
            vec![ids[2].clone()]
        );
        let event = dm::PipelineEventBox::Block(pipeline::BlockEvent {
            header: dm::BlockHeader::new(NonZeroU64::new(5).unwrap(), None, None, 2_000, 0),
            status: dm::BlockStatus::Committed,
        });
        assert_eq!(
            replacement
                .match_pipeline_event(&event, 6)
                .collect::<Vec<_>>(),
            vec![ids[1].clone()]
        );
    }
    assert_eq!(json::to_json(&restored).unwrap(), encoded);
    assert_eq!(json::to_json(&set).unwrap(), encoded);
    restored.block_and_revert().commit();
    assert_eq!(active_trigger_ids(&restored), expected);
    let restarted: Set = json::from_json(&json::to_json(&restored).unwrap()).unwrap();
    assert_eq!(active_trigger_ids(&restarted), expected);
}

#[test]
fn set_json_rejects_invalid_scope_authorization_in_predecessor() {
    let mut set = sample_set();
    let (current, key, mut prior) = {
        let view = set.data_triggers.view();
        let (key, value) = view.iter().next().unwrap();
        (
            view.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            key.clone(),
            value.clone(),
        )
    };
    prior.metadata = Metadata::default();
    set.data_triggers = Storage::from_snapshot_parts(current, BTreeMap::from([(key, Some(prior))]));
    let encoded = json::to_json(&set).unwrap();
    let Err(error) = json::from_json::<Set>(&encoded) else {
        panic!("invalid prior data trigger was accepted")
    };
    assert!(error.to_string().contains("predecessor"), "{error}");
    assert!(error.to_string().contains("scope authorization"), "{error}");
    assert_eq!(json::to_json(&set).unwrap(), encoded);
}
