// Stored trigger grammar checks retain current and undo owners for all four action tables.
#[test]
fn ironwood_trigger_payload_guard_checks_all_current_and_retained_action_bodies() {
    use iroha_data_model::{
        isi::privacy::SchedulePrivacyIronwoodLimitsUpdateV1,
    };
    let catalog = crate::runtime::release_catalog().unwrap();
    let successor = &catalog.ironwood.as_ref().unwrap().selection;
    let instruction: InstructionBox = SchedulePrivacyIronwoodLimitsUpdateV1::new(8, iroha_data_model::privacy::OrchardLimitsV1 { max_action_count: iroha_data_model::privacy::ORCHARD_MAX_ACTIONS_V1 })
    .into();
    for undo in [false, true] {
        for batch in [false, true] {
            let executable = if batch {
                ExecutableRef::Batch(
                    vec![ExecutableBatchItem::Instruction(instruction.clone())].into(),
                )
            } else {
                ExecutableRef::Instructions(vec![instruction.clone()].into())
            };
            macro_rules! check {
                ($field:ident) => {{
                    let mut set = sample_set();
                    set.validate_privacy_payload_grammar(&catalog.baseline).unwrap();
                    let (mut current, key, mut action) = {
                        let view = set.$field.view();
                        let (key, action) = view.iter().next().unwrap();
                        (view.iter().map(|(key, action)| (key.clone(), action.clone())).collect::<BTreeMap<_, _>>(), key.clone(), action.clone())
                    };
                    action.executable = executable.clone();
                    let predecessor = if undo {
                        BTreeMap::from([(key, Some(action))])
                    } else {
                        current.insert(key, action);
                        BTreeMap::new()
                    };
                    set.$field = Storage::from_snapshot_parts(current, predecessor);
                    let before = json::to_json(&set).unwrap();
                    assert!(matches!(set.validate_privacy_payload_grammar(&catalog.baseline),
                        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(message))
                        if message.contains("inactive Ironwood payload grammar")));
                    set.validate_privacy_payload_grammar(successor).unwrap();
                    assert_eq!(json::to_json(&set).unwrap(), before, "guard preserves every original current/undo frame");
                }};
            }
            check!(data_triggers);
            check!(pipeline_triggers);
            check!(time_triggers);
            check!(by_call_triggers);
        }
    }
}
