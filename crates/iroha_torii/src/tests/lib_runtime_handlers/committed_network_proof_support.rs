// Actual State execution, native paired attestations, original Kura publication, and portable
// independently selected H1 checkpoint -> H2 verification. The fixture signs the four-seat
// committee's votes itself; it does not qualify a running distributed consensus network.

fn committed_network_proof_app_for_test() -> (
    SharedAppState,
    iroha_data_model::block::SharedSignedBlock,
    iroha_data_model::sumeragi_finality::VerifiedSumeragiBlock,
) {
    use iroha_core::{
        state::World,
        sumeragi::{
            finality::{build_checkpoint, build_proof},
            test_chain::{CertifiedTestChain, TestChainConfig},
        },
    };
    use iroha_data_model::{
        Registrable,
        account::Account,
        events::{
            pipeline::{BlockEventFilter, BlockStatus, PipelineEventFilterBox},
            time::{ExecutionTime, TimeEventFilter},
        },
        isi::{InstructionBox, Log, Register, Unregister},
        level::Level,
        sumeragi_finality::SumeragiFinalityVerifier,
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    let key = checked_torii_test_ed25519_keypair(0x39, "proof fixture input signer");
    let authority = AccountId::new(key.public_key().clone());
    let account = Account::new(authority.clone()).build(&authority);
    let mut config = TestChainConfig::new(World::with([], [account], []), 1_000);
    let instructions =
        |label: &str| vec![InstructionBox::from(Log::new(Level::INFO, label.into()))];
    // Normal signed-genesis registration supplies lifecycle markers. Both invocations become
    // eligible at H2 and are executed by the same production owner as the two Network inputs.
    let pipeline = Trigger::new(
        "torii_proof_pipeline".parse().unwrap(),
        Action::new(
            instructions("proof pipeline callback"),
            Repeats::Exactly(1),
            authority.clone(),
            PipelineEventFilterBox::from(
                BlockEventFilter::new()
                    .for_height(NonZeroU64::new(2).unwrap())
                    .for_status(BlockStatus::Approved),
            ),
        )
        .unwrap(),
    );
    let timer = Trigger::new(
        "torii_proof_timer".parse().unwrap(),
        Action::new(
            instructions("proof timer callback"),
            Repeats::Exactly(1),
            authority.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .unwrap(),
    );
    config.genesis_instructions.extend([
        InstructionBox::from(Register::trigger(pipeline)),
        InstructionBox::from(Register::trigger(timer)),
    ]);
    let chain_id = config.chain_id.to_string();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let mut transactions = Vec::new();
    for work in [
        instructions("successful proof input"),
        vec![InstructionBox::from(Unregister::domain(
            iroha_model_base::domain::DomainId::try_new("missing_proof_domain", "universal")
                .unwrap(),
        ))],
    ] {
        let mut builder = TransactionBuilder::new(
            chain.network_id(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(std::time::Duration::from_millis(1_001));
        transactions.push(builder.with_instructions(work).sign(key.private_key()));
    }
    assert_eq!(chain.commit(transactions), vec![true, false]);
    let block = chain.committed(2).block().clone();
    assert_eq!(block.network_entrypoint_count(), 2);
    assert_eq!(block.execution_outputs().len(), 4);
    assert!(matches!(
        block.execution_outputs()[2],
        iroha_data_model::block::execution_output::ExecutionOutputV1::Pipeline(_)
    ));
    assert!(matches!(
        block.execution_outputs()[3],
        iroha_data_model::block::execution_output::ExecutionOutputV1::Time(_)
    ));
    let view = chain.state().view();
    let checkpoint = build_checkpoint(&view, 1).unwrap();
    let proof = build_proof(&view, 2).unwrap();
    let verified = SumeragiFinalityVerifier::from_trusted_checkpoint(
        &checkpoint,
        &chain.network_id(),
        &chain_id,
    )
    .unwrap()
    .verify(&proof)
    .unwrap();
    assert_eq!(verified.block(), block.as_ref());
    drop(view);
    let mut app = mk_app_state_for_tests();
    let unique = Arc::get_mut(&mut app).unwrap();
    unique.state = chain.state().clone();
    unique.kura = chain.kura().clone();
    (app, block, verified)
}
