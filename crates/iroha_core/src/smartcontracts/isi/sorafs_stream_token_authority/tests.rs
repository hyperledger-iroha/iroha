//! Exact direct External provenance and sealed/nested fail-closed role-11 regressions.

use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    sorafs::stream_token_authority::{
        StreamTokenAuthorityRequestV1, StreamTokenExpireV1, StreamTokenReviewedV1,
    },
    transaction::{FeePaymentIntent, TransactionBuilder, signed::SealedTransactionReveal},
};
use sorafs_manifest::signer::{
    protocol::{
        SignerOperationActionV1, SignerOperationAuditHeadV1, SignerOperationCustodyV1,
        SignerOperationIntentV1, SignerOperationReservationV1,
    },
    stream_token::SignerStreamTokenRequestV1,
};

#[test]
fn direct_signed_role11_has_exact_entry_and_instruction_position_only() {
    let key = KeyPair::try_from_seed(vec![0x41; 32], Algorithm::Ed25519).unwrap();
    let authority = AccountId::new(key.public_key().clone());
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let instruction = MutateSorafsStreamTokenAuthority {
        request: StreamTokenAuthorityRequestV1 {
            network_id: *state.network_id.as_bytes(),
            provider_id: ProviderId::new([0x42; 32]),
            expected_control_revision: 1,
            expected_control_digest: [0x43; 32],
            action: Action::Expire(StreamTokenExpireV1 {
                operation_id: [0x44; 32],
                reservation: sorafs_manifest::signer::protocol::SignerOperationReservationV1 {
                    reservation_id: [0x45; 32],
                    fence: 1,
                    expires_at_unix_ms: 2_000,
                },
            }),
        },
    };
    let signed = TransactionBuilder::new(
        state.network_id,
        authority.clone(),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([instruction])
    .sign(key.private_key());
    let external = TransactionEntrypoint::External(signed.clone());
    let sealed = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"role11 sealed reveal commitment"),
        signed.clone(),
        [0x46; 32],
    ));
    assert_ne!(external.hash(), sealed.hash());
    let mut block = state.block(BlockHeader::new(
        1_u64.try_into().unwrap(),
        None,
        None,
        1_000,
        0,
    ));
    let mut tx = block.transaction();
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_tx_hash = Some(signed.hash());
    tx.current_entrypoint_index = Some(7);
    tx.current_network_entrypoint_hash = Some(external.hash());
    tx.current_direct_stream_token_instruction_index = Some(0);
    let exact = execution(&mut tx, &authority).expect("direct signed entry");
    assert_eq!(exact.transaction_hash, *external.hash().as_ref());
    assert_eq!((exact.entry_index, exact.instruction_index), (7, 0));
    assert_eq!(tx.current_direct_stream_token_instruction_index, None);

    tx.current_direct_stream_token_instruction_index = Some(0);
    tx.current_network_entrypoint_hash = Some(sealed.hash());
    assert_eq!(execution(&mut tx, &authority), Err(Error::Execution));
    assert_eq!(tx.current_direct_stream_token_instruction_index, None);

    tx.current_network_entrypoint_hash = Some(external.hash());
    assert_eq!(execution(&mut tx, &authority), Err(Error::Execution));
}

#[test]
fn same_signed_transaction_cannot_stage_a_terminal_row_or_mutate_state() {
    let operator = AccountId::new(
        KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let manager = AccountId::new(
        KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    );
    let provider = ProviderId::new([0x53; 32]);
    let request = SignerStreamTokenRequestV1 {
        operation_id: [0x54; 32],
        binding_digest: [0x55; 32],
        original_custody: SignerOperationCustodyV1 {
            record_digest: [0x56; 32],
            control_state_digest: [0x57; 32],
        },
        signing_payload_digest: [0x58; 32],
        signing_payload_size: 512,
        issued_at_unix_ms: 500,
        expires_at_unix_ms: 2_000,
    };
    let reviewed = StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: request.operation_id,
            request_digest: request.digest().unwrap(),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        },
    };
    let reserved_execution = StreamTokenExecutionV1 {
        height: 1,
        transaction_hash: [0x59; 32],
        entry_index: 0,
        instruction_index: 0,
        recorded_at_unix_ms: 1_000,
        authority: operator,
    };
    let mut terminal = reserved_execution.clone();
    terminal.instruction_index = 1;
    terminal.recorded_at_unix_ms = 1_100;
    terminal.authority = manager;
    let mut row = StreamTokenNativeOperationV1 {
        provider_id: provider,
        custody_control_revision: 1,
        custody_control_digest: [0x57; 32],
        operation: StreamTokenOperationV1 {
            reviewed,
            reservation: SignerOperationReservationV1 {
                reservation_id: [0x5a; 32],
                fence: 1,
                expires_at_unix_ms: 1_500,
            },
            outcome: StreamTokenOutcomeV1::Expired,
        },
        reserved_execution,
        terminal_execution: Some(terminal),
    };
    row.terminal_execution.as_mut().unwrap().transaction_hash = [0x5b; 32];
    assert_eq!(
        validate_stream_token_native_operation_claim_v1(
            &row,
            provider,
            1,
            [0x57; 32],
            &row.reserved_execution.authority,
        ),
        Ok(())
    );
    row.terminal_execution.as_mut().unwrap().transaction_hash = [0x59; 32];
    let record = OperationRecordV1 {
        revision: 2,
        predecessor_digest: [0x5c; 32],
        request_digest: [0x5d; 32],
        operation: row,
    };
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(BlockHeader::new(
        1_u64.try_into().unwrap(),
        None,
        None,
        1_100,
        0,
    ));
    let mut tx = block.transaction();
    let next = OperationHeadV1 {
        revision: 2,
        digest: [0x5e; 32],
        fence: 1,
        audit: reviewed.intent.previous_audit,
        active_operation: None,
        total_admissions: 1,
    };
    assert_eq!(
        stage_transition(&mut tx, provider, record, next, false),
        Err(Error::Invalid)
    );
    assert!(
        tx.world
            .smart_contract_state
            .get(&journal::head_key(provider))
            .is_none()
    );
    assert!(
        tx.world
            .smart_contract_state
            .get(&journal::record_key(provider, 2))
            .is_none()
    );
}
