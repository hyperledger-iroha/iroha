//! Same-State role-11 history-pair regressions; these fixtures are not finality evidence.

use super::*;
use crate::state::World;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    sorafs::stream_token_authority::{
        StreamTokenExecutionV1, StreamTokenOperationV1, StreamTokenReviewedV1,
    },
};
use sorafs_manifest::signer::{
    protocol::{
        SignerOperationActionV1, SignerOperationCustodyV1, SignerOperationIntentV1,
        SignerOperationReservationV1,
    },
    stream_token::SignerStreamTokenRequestV1,
};

fn operator() -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519)
            .unwrap()
            .public_key()
            .clone(),
    )
}

fn reserve(
    provider: ProviderId,
    id: [u8; 32],
    revision: u64,
    fence: u64,
    previous_digest: [u8; 32],
    height: u64,
) -> OperationRecordV1 {
    let request = SignerStreamTokenRequestV1 {
        operation_id: id,
        binding_digest: [0x52; 32],
        original_custody: SignerOperationCustodyV1 {
            record_digest: [0x53; 32],
            control_state_digest: [0x54; 32],
        },
        signing_payload_digest: [0x55; 32],
        signing_payload_size: 256,
        issued_at_unix_ms: 900,
        expires_at_unix_ms: 5_000,
    };
    let reviewed = StreamTokenReviewedV1 {
        request,
        intent: SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: id,
            request_digest: request.digest().unwrap(),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        },
    };
    OperationRecordV1 {
        revision,
        predecessor_digest: previous_digest,
        request_digest: [0x56; 32],
        operation: StreamTokenNativeOperationV1 {
            provider_id: provider,
            custody_control_revision: 1,
            custody_control_digest: [0x54; 32],
            operation: StreamTokenOperationV1 {
                reviewed,
                reservation: SignerOperationReservationV1 {
                    reservation_id: [0x57; 32],
                    fence,
                    expires_at_unix_ms: 3_500,
                },
                outcome: StreamTokenOutcomeV1::Reserved,
            },
            reserved_execution: StreamTokenExecutionV1 {
                height,
                transaction_hash: [u8::try_from(0x58 + height).unwrap(); 32],
                entry_index: 0,
                instruction_index: 0,
                recorded_at_unix_ms: 1_000 + height * 500,
                authority: operator(),
            },
            terminal_execution: None,
        },
    }
}

fn expired(
    original: &OperationRecordV1,
    revision: u64,
    predecessor_digest: [u8; 32],
) -> OperationRecordV1 {
    let mut current = original.clone();
    current.revision = revision;
    current.predecessor_digest = predecessor_digest;
    current.request_digest = [0x59; 32];
    current.operation.operation.outcome = StreamTokenOutcomeV1::Expired;
    current.operation.terminal_execution = Some(StreamTokenExecutionV1 {
        height: revision,
        transaction_hash: [0x5a; 32],
        entry_index: 0,
        instruction_index: 0,
        recorded_at_unix_ms: 2_000,
        authority: operator(),
    });
    current
}

fn world_with_buried_terminal(
    terminal_revision: u64,
    terminal_predecessor: [u8; 32],
    change_terminal: impl FnOnce(&mut OperationRecordV1),
) -> (
    World,
    ProviderId,
    [u8; 32],
    OperationRecordV1,
    OperationRecordV1,
) {
    let provider = ProviderId::new([0x61; 32]);
    let old_id = [0x62; 32];
    let new_id = [0x63; 32];
    let original = reserve(provider, old_id, 1, 1, [0; 32], 1);
    let mut terminal = expired(&original, terminal_revision, terminal_predecessor);
    change_terminal(&mut terminal);
    let next_revision = terminal_revision + 1;
    let next = reserve(
        provider,
        new_id,
        next_revision,
        2,
        record_digest(&terminal).unwrap(),
        next_revision,
    );
    let mut world = World::new();
    for record in [&original, &terminal, &next] {
        world.smart_contract_state.insert(
            record_key(provider, record.revision),
            encode(record).unwrap(),
        );
    }
    world
        .smart_contract_state
        .insert(admission_key(provider, old_id), encode(&1_u64).unwrap());
    world.smart_contract_state.insert(
        slot_key(provider, old_id),
        encode(&terminal_revision).unwrap(),
    );
    world.smart_contract_state.insert(
        admission_key(provider, new_id),
        encode(&next_revision).unwrap(),
    );
    world
        .smart_contract_state
        .insert(slot_key(provider, new_id), encode(&next_revision).unwrap());
    world.smart_contract_state.insert(
        head_key(provider),
        encode(&OperationHeadV1 {
            revision: next_revision,
            digest: record_digest(&next).unwrap(),
            fence: 2,
            audit: next.operation.operation.reviewed.intent.previous_audit,
            active_operation: Some(new_id),
            total_admissions: 2,
        })
        .unwrap(),
    );
    (world, provider, old_id, original, terminal)
}

#[test]
fn history_pair_returns_original_reserve_and_adjacent_terminal() {
    let provider = ProviderId::new([0x61; 32]);
    let original = reserve(provider, [0x62; 32], 1, 1, [0; 32], 1);
    let (world, provider, id, original, terminal) =
        world_with_buried_terminal(2, record_digest(&original).unwrap(), |_| {});
    let view = world.view();
    let history = read_history(&view, provider, id).unwrap().unwrap();
    assert_eq!(history.reserved, original);
    assert_eq!(history.current, terminal);
    assert_eq!(read_slot(&view, provider, id).unwrap(), Some(terminal));
}

#[test]
fn history_pair_rejects_terminal_revision_gap_buried_below_head() {
    let provider = ProviderId::new([0x61; 32]);
    let original = reserve(provider, [0x62; 32], 1, 1, [0; 32], 1);
    let (world, provider, id, _, _) =
        world_with_buried_terminal(3, record_digest(&original).unwrap(), |_| {});
    let view = world.view();
    assert!(read_head(&view, provider).is_ok());
    assert_eq!(
        read_history(&view, provider, id),
        Err(Error::CorruptHistory)
    );
    assert_eq!(read_slot(&view, provider, id), Err(Error::CorruptHistory));
}

#[test]
fn history_pair_rejects_wrong_terminal_predecessor_buried_below_head() {
    let (world, provider, id, _, _) = world_with_buried_terminal(2, [0x99; 32], |_| {});
    let view = world.view();
    assert!(read_head(&view, provider).is_ok());
    assert_eq!(
        read_history(&view, provider, id),
        Err(Error::CorruptHistory)
    );
    assert_eq!(read_slot(&view, provider, id), Err(Error::CorruptHistory));
}

#[test]
fn history_pair_rejects_buried_terminal_custody_generation_substitution() {
    let provider = ProviderId::new([0x61; 32]);
    let original = reserve(provider, [0x62; 32], 1, 1, [0; 32], 1);
    for change_revision in [true, false] {
        let (world, provider, id, _, _) =
            world_with_buried_terminal(2, record_digest(&original).unwrap(), |terminal| {
                if change_revision {
                    terminal.operation.custody_control_revision = 2;
                } else {
                    terminal.operation.custody_control_digest = [0x98; 32];
                }
            });
        let view = world.view();
        assert!(read_head(&view, provider).is_ok());
        assert_eq!(
            read_history(&view, provider, id),
            Err(Error::CorruptHistory)
        );
        assert_eq!(read_slot(&view, provider, id), Err(Error::CorruptHistory));
    }
}
