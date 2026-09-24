// Exact signed source linkage for the one-use role-11 instruction ordinal.

use iroha_data_model::{
    isi::sorafs::MutateSorafsStreamTokenAuthority,
    sorafs::{
        capacity::ProviderId,
        stream_token_authority::{
            StreamTokenAuthorityActionV1, StreamTokenAuthorityRequestV1, StreamTokenExpireV1,
        },
    },
    transaction::signed::SealedTransactionReveal,
};

#[test]
fn role11_ordinal_requires_the_exact_signed_instruction_and_outer_entry() {
    let key = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(key.public_key().clone());
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        query::store::LiveQueryStore::start_test(),
    );
    let role11 = MutateSorafsStreamTokenAuthority {
        request: StreamTokenAuthorityRequestV1 {
            network_id: *state.network_id.as_bytes(),
            provider_id: ProviderId::new([0x72; 32]),
            expected_control_revision: 1,
            expected_control_digest: [0x73; 32],
            action: StreamTokenAuthorityActionV1::Expire(StreamTokenExpireV1 {
                operation_id: [0x74; 32],
                reservation: sorafs_manifest::signer::protocol::SignerOperationReservationV1 {
                    reservation_id: [0x75; 32],
                    fence: 1,
                    expires_at_unix_ms: 2_000,
                },
            }),
        },
    };
    let direct: InstructionBox = role11.clone().into();
    let signed = TransactionBuilder::new(
        state.network_id,
        authority,
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([role11.clone()])
    .sign(key.private_key());
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1, 0));
    let mut tx = block.transaction();
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(signed.hash());
    tx.current_entrypoint_index = Some(3);
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(&tx, &signed, &direct, 0, true)
            .unwrap(),
        Some(0)
    );
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(&tx, &signed, &direct, 0, false)
            .unwrap(),
        None
    );
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(&tx, &signed, &direct, 1, true)
            .unwrap(),
        None
    );
    let mut altered = role11.clone();
    altered.request.expected_control_digest = [0x76; 32];
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(
            &tx,
            &signed,
            &InstructionBox::from(altered),
            0,
            true,
        )
        .unwrap(),
        None
    );
    let sealed = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"role11 sealed outer identity"),
        signed.clone(),
        [0x77; 32],
    ));
    tx.current_network_entrypoint_hash = Some(sealed.hash());
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(&tx, &signed, &direct, 0, true)
            .unwrap(),
        None
    );

    let mixed = TransactionBuilder::new(
        state.network_id,
        signed.authority().clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_executable(Executable::Batch(
        vec![
            ExecutableBatchItem::Instruction(
                iroha_data_model::isi::Log::new(Level::INFO, "first".to_owned()).into(),
            ),
            ExecutableBatchItem::Instruction(direct.clone()),
        ]
        .into(),
    ))
    .sign(key.private_key());
    tx.current_network_entrypoint_hash = Some(mixed.hash_as_entrypoint());
    tx.tx_call_hash = Some(Hash::from(mixed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(mixed.hash());
    assert_eq!(
        super::Executor::direct_stream_token_instruction_index(&tx, &mixed, &direct, 1, true)
            .unwrap(),
        Some(1)
    );
}
