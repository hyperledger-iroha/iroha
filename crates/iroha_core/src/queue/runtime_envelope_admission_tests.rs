// Honest proposers never carry an envelope the committed selection does not admit
// (`specs/live_runtime_upgrades.md` §5.2).

#[test]
fn admission_refuses_an_envelope_the_committed_selection_does_not_bind() {
    let (mut state, time) = current_admission_queue_fixture();
    let unknown = "test.unreleased.instruction.v1";
    let log = || {
        InstructionBox::from(iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "admission".into(),
        ))
    };
    let opaque = InstructionBox::from(
        iroha_data_model::isi::OpaqueInstructionV1::new(unknown, vec![0xA5; 16])
            .expect("well-formed wire id"),
    );
    let refused = accepted_tx_with(
        AccountId::new(ALICE_KEYPAIR.public_key().clone()),
        &ALICE_KEYPAIR,
        &time,
        vec![log(), opaque],
        Metadata::default(),
    );
    register_accepted_tx_authority_for_queue_test(&mut state, &refused);
    let queue = Queue::test(config_factory(), &time);
    let failure = queue
        .push(refused, state.view())
        .expect_err("an unbound envelope is refused before queue custody");
    assert!(
        matches!(
            &failure.err,
            Error::RuntimeRejected(
                iroha_data_model::runtime::lifecycle::RuntimeRejectionV1::DefinitionInactive {
                    wire_id,
                }
            ) if wire_id == unknown
        ),
        "{:?}",
        failure.err
    );
    assert!(failure.err.to_string().contains(unknown));
    assert_eq!((queue.active_len(), queue.queued_len()), (0, 0));
    let admitted = accepted_tx_with(
        AccountId::new(ALICE_KEYPAIR.public_key().clone()),
        &ALICE_KEYPAIR,
        &time,
        vec![log()],
        Metadata::default(),
    );
    queue
        .push(admitted, state.view())
        .expect("bound envelopes are admitted");
    assert_eq!((queue.active_len(), queue.queued_len()), (1, 1));
}
