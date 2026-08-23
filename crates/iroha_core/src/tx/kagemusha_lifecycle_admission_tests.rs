fn lifecycle_cancellation_instruction() -> InstructionBox {
    use iroha_data_model::{
        isi::offline::CancelKagemushaRecursiveReleaseV4,
        offline::{
            KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1, KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            KagemushaExactBytesDigestV1, KagemushaV4ReleaseCancellationV1,
            KagemushaV4ReleaseLifecycleReasonV1,
        },
    };

    InstructionBox::from(CancelKagemushaRecursiveReleaseV4::new(
        KagemushaV4ReleaseCancellationV1 {
            schema: KAGEMUSHA_V4_RELEASE_CANCELLATION_SCHEMA_V1.to_owned(),
            version: KAGEMUSHA_V4_RELEASE_LIFECYCLE_VERSION_V1,
            promotion_id: [0x11; 32],
            manifest_sha256: [0x22; 32],
            expected_predecessor_lifecycle: KagemushaExactBytesDigestV1 {
                byte_len: 1,
                sha256: [0x33; 32],
            },
            transition_id: [0x44; 32],
            reason: KagemushaV4ReleaseLifecycleReasonV1::GovernanceCancelled,
            evidence: None,
        },
    ))
}

#[test]
fn direct_kagemusha_lifecycle_authority_requires_one_exact_instruction() {
    let cancellation = lifecycle_cancellation_instruction();
    let ordinary = InstructionBox::from(Log::new(Level::INFO, "ordinary".into()));

    assert!(instructions_allow_direct_kagemusha_lifecycle_authority(
        core::slice::from_ref(&cancellation)
    ));
    assert!(!instructions_allow_direct_kagemusha_lifecycle_authority(&[]));
    assert!(!instructions_allow_direct_kagemusha_lifecycle_authority(
        core::slice::from_ref(&ordinary)
    ));
    assert!(!instructions_allow_direct_kagemusha_lifecycle_authority(&[
        cancellation,
        ordinary,
    ]));
}

#[test]
fn exact_kagemusha_lifecycle_accepts_verified_multisig_authority_at_stateful_admission() {
    let member_a = checked_random_tx_keypair();
    let member_b = checked_random_tx_keypair();
    let authority = AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(member_a.public_key().clone(), 1).expect("member a"),
                MultisigMember::new(member_b.public_key().clone(), 1).expect("member b"),
            ],
        )
        .expect("multisig lifecycle authority"),
    );
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "multisig-kagemusha-lifecycle".parse().unwrap(),
    );
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([lifecycle_cancellation_instruction()])
    .sign_multisig([member_a.private_key(), member_b.private_key()]);
    let accepted = AcceptedTransaction::accept(
        tx,
        &test_network_id(),
        Duration::ZERO,
        TransactionParameters::default(),
        &iroha_config::parameters::actual::Crypto::default(),
    )
    .expect("multisig lifecycle signatures must verify before stateful admission");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();

    StateBlock::validate_stateful_admission(accepted.as_ref(), &mut state_transaction, None)
        .expect("exact lifecycle carrier must pass the narrow multisig admission exception");
}

#[test]
fn exact_kagemusha_lifecycle_rejects_one_threshold_weight_signer_at_stateful_admission() {
    let member_a = checked_random_tx_keypair();
    let member_b = checked_random_tx_keypair();
    let authority = AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(member_a.public_key().clone(), 2).expect("member a"),
                MultisigMember::new(member_b.public_key().clone(), 1).expect("member b"),
            ],
        )
        .expect("structurally valid weighted lifecycle authority"),
    );
    let world = World::with([], [Account::new(authority.clone()).build(&authority)], []);
    let state = State::new_with_chain(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "weighted-kagemusha-lifecycle".parse().unwrap(),
    );
    let tx = TransactionBuilder::new(
        test_network_id(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([lifecycle_cancellation_instruction()])
    .sign_multisig([member_a.private_key()]);
    let accepted = AcceptedTransaction::accept(
        tx,
        &test_network_id(),
        Duration::ZERO,
        TransactionParameters::default(),
        &iroha_config::parameters::actual::Crypto::default(),
    )
    .expect("the generic weighted threshold accepts member A alone");
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut state_transaction = block.transaction();

    let error =
        StateBlock::validate_stateful_admission(accepted.as_ref(), &mut state_transaction, None)
            .expect_err("Kagemusha lifecycle admission requires two distinct signers");
    assert!(
        error
            .to_string()
            .contains("requires at least 2 verified distinct governance signers"),
        "unexpected rejection: {error}"
    );
}
