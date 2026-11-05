// Direct native trigger registration obeys the same selection grammar as admission and restore.
#[test]
fn ironwood_trigger_registration_requires_the_successor_selection() {
    use iroha_data_model::{
        isi::privacy::SchedulePrivacyIronwoodLimitsUpdateV1,
    };
    for activated in [false, true] {
        let mut world = component_trigger_world();
        if activated {
            crate::runtime::ironwood::install_for_test(&mut world);
        }
        let state = State::new(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let block = new_dummy_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut transaction = state_block.transaction();
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
            let instruction: InstructionBox = SchedulePrivacyIronwoodLimitsUpdateV1::new(8, iroha_data_model::privacy::OrchardLimitsV1 { max_action_count: iroha_data_model::privacy::ORCHARD_MAX_ACTIONS_V1 })
        .into();
        let id: TriggerId = "ironwood_payload".parse().unwrap();
        let trigger = Trigger::new(
            id.clone(),
            Action::new(
                vec![instruction],
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                ExecuteTriggerEventFilter::new(),
            )
            .unwrap(),
        );
        let result = Register::trigger(trigger).execute(&ALICE_ID, &mut transaction);
        if activated {
            result.expect("the successor grammar admits the typed stored body");
            assert!(
                transaction
                    .world
                    .triggers
                    .inspect_by_id(&id, |_| ())
                    .is_some()
            );
        } else {
            let error = result.expect_err("predecessor storage cannot retain ordinal 12");
            assert_smart_contract_error_contains(&error, "inactive Ironwood payload grammar");
            assert!(
                transaction
                    .world
                    .triggers
                    .inspect_by_id(&id, |_| ())
                    .is_none()
            );
        }
    }
}
