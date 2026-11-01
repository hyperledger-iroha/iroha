/// Transaction validation stops at the first top-level envelope the selection does not bind,
/// before admission, routing, fee and policy steps (`specs/live_runtime_upgrades.md` §5.2).
#[test]
fn state_rejects_an_unbound_envelope_before_instruction_dependent_steps() {
    let chain: ChainId = "runtime-envelope-chain".parse().unwrap();
    let (world, authority, keypair) = world_with_authority("wonderland");
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        chain,
    );
    let unknown = "test.unreleased.instruction.v1";
    let opaque = InstructionBox::from(
        iroha_data_model::isi::OpaqueInstructionV1::new(unknown, vec![0xA5; 16])
            .expect("well-formed wire id"),
    );
    let transaction = |instructions: Vec<InstructionBox>| {
        TransactionBuilder::new(
            test_network_id(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(keypair.private_key())
    };
    let inactive = |wire_id: &str| {
        Err(TransactionRejectionReason::Validation(
            ValidationFail::InstructionFailed(
                iroha_data_model::isi::error::InstructionExecutionError::Runtime(
                    iroha_data_model::runtime::lifecycle::RuntimeRejectionV1::DefinitionInactive {
                        wire_id: wire_id.to_owned(),
                    },
                ),
            ),
        ))
    };
    let log = || InstructionBox::from(Log::new(Level::INFO, "envelope".into()));
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 0, 0));
    let mut ivm_cache = IvmCache::new();

    let unknown_tx = transaction(vec![log(), opaque]);
    let result = execute_component_transaction_for_testing(
        &mut block,
        AcceptedTransaction::new_unchecked(Cow::Owned(unknown_tx)),
        &mut ivm_cache,
        None,
    );
    assert_eq!(result.map(|_| ()), inactive(unknown));

    // A wire ID this release registers is refused alike while the selection does not bind it.
    let log_wire_id = iroha_data_model::isi::wire_id_of::<Log>().expect("registered");
    block
        .world
        .runtime_selection
        .get_mut()
        .bindings
        .retain(|binding| binding.wire_id != log_wire_id);
    let unbound_tx = transaction(vec![log()]);
    let result = execute_component_transaction_for_testing(
        &mut block,
        AcceptedTransaction::new_unchecked(Cow::Owned(unbound_tx)),
        &mut ivm_cache,
        None,
    );
    assert_eq!(result.map(|_| ()), inactive(log_wire_id));
}
