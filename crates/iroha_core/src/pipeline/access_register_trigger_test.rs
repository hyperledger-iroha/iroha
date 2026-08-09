    #[test]
    fn register_trigger_keys_cover_definition_and_repetitions() {
        use iroha_primitives::const_vec::ConstVec;
        let (alice, alice_keypair) = iroha_test_samples::gen_account_in("wonderland");
        let trig: TriggerId = "t_reg".parse().unwrap();
        let trigger = Trigger::new(
            trig.clone(),
            Action::new(
                ConstVec::<InstructionBox>::new_empty(),
                Repeats::Exactly(1),
                alice.clone(),
                ExecuteTriggerEventFilter::new()
                    .for_trigger(trig.clone())
                    .under_authority(alice.clone()),
            )
            .expect("trigger action fixture satisfies validation invariants"),
        );
        let tx = TransactionBuilder::new(
            "chain".parse().unwrap(),
            alice,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([InstructionBox::from(Register::trigger(trigger))])
        .sign(alice_keypair.private_key());
        let set = derive_for_transaction::<crate::state::StateView<'_>>(
            &tx,
            None,
            IvmStrategy::Conservative,
        );
        assert!(set.read_keys.contains(&format!("trigger:{trig}")));
        assert!(set.write_keys.contains(&format!("trigger:{trig}")));
        assert!(
            set.write_keys
                .contains(&format!("trigger.repetitions:{trig}"))
        );
    }
