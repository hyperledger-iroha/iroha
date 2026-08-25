//! Private Offline Cash STATE balance/credit relation.
//!
//! The witness is deliberately move-only and zeroizes every private byte and
//! amount on all exits, including constructor rejection and failed synthesis.
//! This module also owns the one canonical framed byte encoding used by both
//! the Core state machine and the Halo2 relation.

use core::fmt;

use iroha_data_model::{
    NetworkId,
    asset::AssetDefinitionId,
    offline::{OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashTransferStatementV1},
};
use sha2::{Digest as _, Sha256};
use zeroize::{Zeroize, Zeroizing};

use super::state_abi::{
    OfflineCashStateAbiErrorV1, OfflineCashStateLeafPublicInstancesV1, OfflineCashStateOperationV1,
    OfflineCashStatePublicInstancesV1, OfflineCashStateRelationPublicV1,
};
use super::state_transition::{STATE_CONTEXT_MESSAGE_BYTES_V1, state_context_message_v1};

#[path = "state_relation_circuit.rs"]
pub(super) mod circuit;

pub(super) const BALANCE_HEAD_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:private-balance";
pub(super) const CREDIT_HEAD_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:private-credit";
pub(super) const RECEIVE_OPENING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:receive-fold-secret";
pub(super) const RECEIVE_TRANSITION_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:receive-fold-transition";
pub(super) const RECEIVE_SEMANTIC_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:receive-fold-statement";
pub(super) const SEND_SPLIT_SEED_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:send-split-seed";
pub(super) const SEND_SPLIT_BRANCH_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:send-split-branch";
pub(super) const SEND_SPLIT_SENDER_BRANCH_V1: &[u8] = b"sender";
pub(super) const SEND_SPLIT_RECEIVER_BRANCH_V1: &[u8] = b"receiver";
pub(super) const STATE_LINEAGE_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:state-lineage";
pub(super) const STATE_HEAD_FRAME_VERSION_V1: u16 = OFFLINE_CASH_WIRE_VERSION_V1;

/// Exact canonical Norito frame bytes for the fixed-width network identity.
pub(super) const NETWORK_ID_FRAME_BYTES_V1: usize = 72;
/// Exact canonical Norito frame bytes for the UUID-backed asset identity.
pub(super) const ASSET_ID_FRAME_BYTES_V1: usize = 72;
/// Exact canonical Norito/SHA-256 SendSplit transition message bytes.
pub(super) const SEND_TRANSITION_MESSAGE_BYTES_V1: usize = 441;
/// Exact canonical Norito/SHA-256 SendSplit semantic message bytes.
pub(super) const SEND_SEMANTIC_MESSAGE_BYTES_V1: usize = 421;
const SEND_TRANSITION_DIGEST_PREFIX_BYTES_V1: usize = 52;
pub(super) const SEND_SEMANTIC_DIGEST_PREFIX_BYTES_V1: usize = 51;
pub(super) const NORITO_FRAME_PAYLOAD_OFFSET_V1: usize = 40;
pub(super) const SEND_TRANSITION_RELEASE_OFFSET_V1: usize = 156;
pub(super) const SEND_TRANSITION_NETWORK_OFFSET_V1: usize = 189;
pub(super) const SEND_TRANSITION_ASSET_OFFSET_V1: usize = 222;
pub(super) const SEND_TRANSITION_SCALE_OFFSET_V1: usize = 255;
pub(super) const SEND_TRANSITION_AMOUNT_OFFSET_V1: usize = 260;
pub(super) const SEND_TRANSITION_REQUEST_OFFSET_V1: usize = 277;
pub(super) const SEND_TRANSITION_BEFORE_OFFSET_V1: usize = 310;
pub(super) const SEND_TRANSITION_AFTER_OFFSET_V1: usize = 343;
pub(super) const SEND_TRANSITION_RECEIVER_OFFSET_V1: usize = 376;
pub(super) const SEND_TRANSITION_CREDIT_OFFSET_V1: usize = 409;
pub(super) const SEND_SEMANTIC_RELEASE_OFFSET_V1: usize = 103;
pub(super) const SEND_SEMANTIC_NETWORK_OFFSET_V1: usize = 136;
pub(super) const SEND_SEMANTIC_ASSET_OFFSET_V1: usize = 169;
pub(super) const SEND_SEMANTIC_SCALE_OFFSET_V1: usize = 202;
pub(super) const SEND_SEMANTIC_AMOUNT_OFFSET_V1: usize = 207;
pub(super) const SEND_SEMANTIC_REQUEST_OFFSET_V1: usize = 224;
pub(super) const SEND_SEMANTIC_BEFORE_OFFSET_V1: usize = 257;
pub(super) const SEND_SEMANTIC_AFTER_OFFSET_V1: usize = 290;
pub(super) const SEND_SEMANTIC_RECEIVER_OFFSET_V1: usize = 323;
pub(super) const SEND_SEMANTIC_CREDIT_OFFSET_V1: usize = 356;
pub(super) const SEND_SEMANTIC_TRANSITION_OFFSET_V1: usize = 389;

const DIGEST_BYTES: usize = 32;
const AMOUNT_BYTES: usize = 16;
const FRAME_LENGTH_BYTES: usize = 8;
const VERSION_BYTES: usize = 2;
const OPERATION_BYTES: usize = 4;
const SCALE_BYTES: usize = 4;
const CREDIT_BINDING_DIGEST_FIELDS: usize = 4;
const BALANCE_BINDING_DIGEST_FIELDS: usize = 5;

pub(super) const fn framed_head_message_len_v1(domain: &[u8]) -> usize {
    FRAME_LENGTH_BYTES
        + domain.len()
        + FRAME_LENGTH_BYTES
        + VERSION_BYTES
        + CREDIT_BINDING_DIGEST_FIELDS * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
        + FRAME_LENGTH_BYTES
        + AMOUNT_BYTES
        + FRAME_LENGTH_BYTES
        + DIGEST_BYTES
}

pub(super) const BALANCE_HEAD_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + BALANCE_HEAD_DOMAIN_V1.len()
    + FRAME_LENGTH_BYTES
    + VERSION_BYTES
    + BALANCE_BINDING_DIGEST_FIELDS * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + FRAME_LENGTH_BYTES
    + core::mem::size_of::<u64>()
    + FRAME_LENGTH_BYTES
    + AMOUNT_BYTES
    + FRAME_LENGTH_BYTES
    + DIGEST_BYTES;
pub(super) const CREDIT_HEAD_MESSAGE_BYTES_V1: usize =
    framed_head_message_len_v1(CREDIT_HEAD_DOMAIN_V1);
pub(super) const STATE_LINEAGE_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + STATE_LINEAGE_DOMAIN_V1.len()
    + FRAME_LENGTH_BYTES
    + VERSION_BYTES
    + FRAME_LENGTH_BYTES
    + OPERATION_BYTES
    + 6 * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + 2 * (FRAME_LENGTH_BYTES + core::mem::size_of::<u64>())
    + FRAME_LENGTH_BYTES
    + AMOUNT_BYTES;
pub(super) const SEND_SPLIT_SEED_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + SEND_SPLIT_SEED_DOMAIN_V1.len()
    + 7 * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + FRAME_LENGTH_BYTES
    + core::mem::size_of::<u64>()
    + FRAME_LENGTH_BYTES
    + AMOUNT_BYTES;
pub(super) const SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + SEND_SPLIT_BRANCH_DOMAIN_V1.len()
    + FRAME_LENGTH_BYTES
    + DIGEST_BYTES
    + FRAME_LENGTH_BYTES
    + SEND_SPLIT_SENDER_BRANCH_V1.len();
pub(super) const SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + SEND_SPLIT_BRANCH_DOMAIN_V1.len()
    + FRAME_LENGTH_BYTES
    + DIGEST_BYTES
    + FRAME_LENGTH_BYTES
    + SEND_SPLIT_RECEIVER_BRANCH_V1.len();
pub(super) const RECEIVE_OPENING_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + RECEIVE_OPENING_DOMAIN_V1.len()
    + 5 * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + FRAME_LENGTH_BYTES
    + AMOUNT_BYTES;
pub(super) const RECEIVE_TRANSITION_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + RECEIVE_TRANSITION_DOMAIN_V1.len()
    + 6 * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + 2 * (FRAME_LENGTH_BYTES + AMOUNT_BYTES);
pub(super) const RECEIVE_SEMANTIC_MESSAGE_BYTES_V1: usize = FRAME_LENGTH_BYTES
    + RECEIVE_SEMANTIC_DOMAIN_V1.len()
    + FRAME_LENGTH_BYTES
    + VERSION_BYTES
    + FRAME_LENGTH_BYTES
    + OPERATION_BYTES
    + 8 * (FRAME_LENGTH_BYTES + DIGEST_BYTES)
    + FRAME_LENGTH_BYTES
    + AMOUNT_BYTES
    + FRAME_LENGTH_BYTES
    + SCALE_BYTES;

const _: () = assert!(BALANCE_HEAD_MESSAGE_BYTES_V1 == 335);
const _: () = assert!(CREDIT_HEAD_MESSAGE_BYTES_V1 > 256);
const _: () = assert!(CREDIT_HEAD_MESSAGE_BYTES_V1 <= 311);
const _: () = assert!(STATE_LINEAGE_MESSAGE_BYTES_V1 == 361);
const _: () = assert!(SEND_SPLIT_SEED_MESSAGE_BYTES_V1 == 365);
const _: () = assert!(SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1 == 101);
const _: () = assert!(SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1 == 103);
const _: () = assert!(RECEIVE_OPENING_MESSAGE_BYTES_V1 == 273);
const _: () = assert!(RECEIVE_TRANSITION_MESSAGE_BYTES_V1 == 341);
const _: () = assert!(RECEIVE_SEMANTIC_MESSAGE_BYTES_V1 == 430);
const _: () = assert!(NETWORK_ID_FRAME_BYTES_V1 == 40 + DIGEST_BYTES);
const _: () = assert!(ASSET_ID_FRAME_BYTES_V1 == 40 + DIGEST_BYTES);
const _: () = assert!(STATE_CONTEXT_MESSAGE_BYTES_V1 == 257);
const _: () = assert!(SEND_TRANSITION_MESSAGE_BYTES_V1 == 441);
const _: () = assert!(SEND_SEMANTIC_MESSAGE_BYTES_V1 == 421);
const _: () = assert!(SEND_TRANSITION_DIGEST_PREFIX_BYTES_V1 == 43 + 1 + 8);
const _: () = assert!(SEND_SEMANTIC_DIGEST_PREFIX_BYTES_V1 == 42 + 1 + 8);
const _: () = assert!(SEND_TRANSITION_CREDIT_OFFSET_V1 + DIGEST_BYTES == 441);
const _: () = assert!(SEND_SEMANTIC_TRANSITION_OFFSET_V1 + DIGEST_BYTES == 421);

fn append_frame_field(target: &mut Vec<u8>, field: &[u8]) {
    target.extend_from_slice(
        &u64::try_from(field.len())
            .expect("Offline Cash head field length fits u64")
            .to_le_bytes(),
    );
    target.extend_from_slice(field);
}

fn framed_message(domain: &[u8], fields: &[&[u8]], expected_len: usize) -> Zeroizing<Vec<u8>> {
    let mut message = Zeroizing::new(Vec::with_capacity(expected_len));
    append_frame_field(&mut message, domain);
    for field in fields {
        append_frame_field(&mut message, field);
    }
    assert_eq!(message.len(), expected_len, "fixed STATE frame geometry");
    message
}

fn zeroizing_exact_array<const N: usize>(bytes: Vec<u8>) -> Option<[u8; N]> {
    let bytes = Zeroizing::new(bytes);
    bytes.as_slice().try_into().ok()
}

fn framed_head_message(
    domain: &[u8],
    context_digest: &[u8; 32],
    first_binding: &[u8; 32],
    second_binding: &[u8; 32],
    third_binding: &[u8; 32],
    amount: u128,
    opening: &[u8; 32],
) -> Zeroizing<Vec<u8>> {
    let expected_len = framed_head_message_len_v1(domain);
    let version = STATE_HEAD_FRAME_VERSION_V1.to_le_bytes();
    let amount = amount.to_le_bytes();
    framed_message(
        domain,
        &[
            &version,
            context_digest,
            first_binding,
            second_binding,
            third_binding,
            &amount,
            opening,
        ],
        expected_len,
    )
}

/// Exact canonical deterministic receiver-opening preimage.
pub(super) fn receive_opening_message_v1(
    context_digest: &[u8; 32],
    before_opening: &[u8; 32],
    credit_opening: &[u8; 32],
    request_digest: &[u8; 32],
    send_transition_digest: &[u8; 32],
    amount: u128,
) -> Zeroizing<Vec<u8>> {
    let amount = amount.to_le_bytes();
    framed_message(
        RECEIVE_OPENING_DOMAIN_V1,
        &[
            context_digest,
            before_opening,
            credit_opening,
            request_digest,
            send_transition_digest,
            &amount,
        ],
        RECEIVE_OPENING_MESSAGE_BYTES_V1,
    )
}

/// Exact canonical receiver-transition preimage.
#[allow(clippy::too_many_arguments)]
pub(super) fn receive_transition_message_v1(
    context_digest: &[u8; 32],
    balance_parent: &[u8; 32],
    credit_parent: &[u8; 32],
    request_digest: &[u8; 32],
    send_transition_digest: &[u8; 32],
    amount: u128,
    next_amount: u128,
    next_head: &[u8; 32],
) -> Zeroizing<Vec<u8>> {
    let amount = amount.to_le_bytes();
    let next_amount = next_amount.to_le_bytes();
    framed_message(
        RECEIVE_TRANSITION_DOMAIN_V1,
        &[
            context_digest,
            balance_parent,
            credit_parent,
            request_digest,
            send_transition_digest,
            &amount,
            &next_amount,
            next_head,
        ],
        RECEIVE_TRANSITION_MESSAGE_BYTES_V1,
    )
}

/// Exact canonical receiver semantic-statement preimage.
#[allow(clippy::too_many_arguments)]
pub(super) fn receive_semantic_message_v1(
    release_id: &[u8; 32],
    context_digest: &[u8; 32],
    request_digest: &[u8; 32],
    balance_parent: &[u8; 32],
    credit_parent: &[u8; 32],
    next_head: &[u8; 32],
    send_transition_digest: &[u8; 32],
    receive_transition_digest: &[u8; 32],
    amount: u128,
    scale: u32,
) -> Zeroizing<Vec<u8>> {
    let version = OFFLINE_CASH_WIRE_VERSION_V1.to_le_bytes();
    let operation = (OfflineCashStateOperationV1::ReceiveFold as u32).to_le_bytes();
    let amount = amount.to_le_bytes();
    let scale = scale.to_le_bytes();
    framed_message(
        RECEIVE_SEMANTIC_DOMAIN_V1,
        &[
            &version,
            &operation,
            release_id,
            context_digest,
            request_digest,
            balance_parent,
            credit_parent,
            next_head,
            send_transition_digest,
            receive_transition_digest,
            &amount,
            &scale,
        ],
        RECEIVE_SEMANTIC_MESSAGE_BYTES_V1,
    )
}

/// Exact canonical balance-head preimage.
pub(super) fn balance_head_message_v1(
    context_digest: &[u8; 32],
    wallet_binding: &[u8; 32],
    guard_device_id: &[u8; 32],
    hardware_policy_id: &[u8; 32],
    guard_sequence: u64,
    lineage_digest: &[u8; 32],
    amount: u128,
    opening: &[u8; 32],
) -> Zeroizing<Vec<u8>> {
    let version = STATE_HEAD_FRAME_VERSION_V1.to_le_bytes();
    let sequence = guard_sequence.to_le_bytes();
    let amount = amount.to_le_bytes();
    framed_message(
        BALANCE_HEAD_DOMAIN_V1,
        &[
            &version,
            context_digest,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            &sequence,
            lineage_digest,
            &amount,
            opening,
        ],
        BALANCE_HEAD_MESSAGE_BYTES_V1,
    )
}

/// Exact non-circular successor-lineage preimage shared by Send and Receive.
#[allow(clippy::too_many_arguments)]
pub(super) fn state_lineage_message_v1(
    operation: OfflineCashStateOperationV1,
    context_digest: &[u8; 32],
    current_head: &[u8; 32],
    current_lineage_digest: &[u8; 32],
    from_sequence: u64,
    to_sequence: u64,
    request_digest: &[u8; 32],
    parent_1: &[u8; 32],
    link: &[u8; 32],
    amount: u128,
) -> Zeroizing<Vec<u8>> {
    let version = STATE_HEAD_FRAME_VERSION_V1.to_le_bytes();
    let operation = (operation as u32).to_le_bytes();
    let from_sequence = from_sequence.to_le_bytes();
    let to_sequence = to_sequence.to_le_bytes();
    let amount = amount.to_le_bytes();
    framed_message(
        STATE_LINEAGE_DOMAIN_V1,
        &[
            &version,
            &operation,
            context_digest,
            current_head,
            current_lineage_digest,
            &from_sequence,
            &to_sequence,
            request_digest,
            parent_1,
            link,
            &amount,
        ],
        STATE_LINEAGE_MESSAGE_BYTES_V1,
    )
}

/// Exact deterministic sender split-seed preimage.
#[allow(clippy::too_many_arguments)]
pub(super) fn send_split_seed_message_v1(
    context_digest: &[u8; 32],
    wallet_binding: &[u8; 32],
    current_head: &[u8; 32],
    current_opening: &[u8; 32],
    guard_sequence: u64,
    request_digest: &[u8; 32],
    receiver_head: &[u8; 32],
    recipient_key_reference: &[u8; 32],
    amount: u128,
) -> Zeroizing<Vec<u8>> {
    let guard_sequence = guard_sequence.to_le_bytes();
    let amount = amount.to_le_bytes();
    framed_message(
        SEND_SPLIT_SEED_DOMAIN_V1,
        &[
            context_digest,
            wallet_binding,
            current_head,
            current_opening,
            &guard_sequence,
            request_digest,
            receiver_head,
            recipient_key_reference,
            &amount,
        ],
        SEND_SPLIT_SEED_MESSAGE_BYTES_V1,
    )
}

pub(super) fn send_split_branch_message_v1(
    split_seed: &[u8; 32],
    branch: &'static [u8],
) -> Zeroizing<Vec<u8>> {
    let expected_len = if branch == SEND_SPLIT_SENDER_BRANCH_V1 {
        SEND_SPLIT_SENDER_BRANCH_MESSAGE_BYTES_V1
    } else if branch == SEND_SPLIT_RECEIVER_BRANCH_V1 {
        SEND_SPLIT_RECEIVER_BRANCH_MESSAGE_BYTES_V1
    } else {
        unreachable!("fixed Offline Cash split branch")
    };
    framed_message(
        SEND_SPLIT_BRANCH_DOMAIN_V1,
        &[split_seed, branch],
        expected_len,
    )
}

/// Exact canonical credit-head preimage.
pub(super) fn credit_head_message_v1(
    context_digest: &[u8; 32],
    request_digest: &[u8; 32],
    receiver_head: &[u8; 32],
    recipient_key_reference: &[u8; 32],
    amount: u128,
    opening: &[u8; 32],
) -> Zeroizing<Vec<u8>> {
    framed_head_message(
        CREDIT_HEAD_DOMAIN_V1,
        context_digest,
        request_digest,
        receiver_head,
        recipient_key_reference,
        amount,
        opening,
    )
}

/// Canonical Core/circuit balance head.
pub(super) fn offline_cash_balance_head_v1(
    context_digest: &[u8; 32],
    wallet_binding: &[u8; 32],
    guard_device_id: &[u8; 32],
    hardware_policy_id: &[u8; 32],
    guard_sequence: u64,
    lineage_digest: &[u8; 32],
    amount: u128,
    opening: &[u8; 32],
) -> [u8; 32] {
    let message = balance_head_message_v1(
        context_digest,
        wallet_binding,
        guard_device_id,
        hardware_policy_id,
        guard_sequence,
        lineage_digest,
        amount,
        opening,
    );
    Sha256::digest(message.as_slice()).into()
}

/// Canonical non-circular lineage committed by the successor balance head.
#[allow(clippy::too_many_arguments)]
pub(super) fn offline_cash_state_lineage_digest_v1(
    operation: OfflineCashStateOperationV1,
    context_digest: &[u8; 32],
    current_head: &[u8; 32],
    current_lineage_digest: &[u8; 32],
    from_sequence: u64,
    to_sequence: u64,
    request_digest: &[u8; 32],
    parent_1: &[u8; 32],
    link: &[u8; 32],
    amount: u128,
) -> [u8; 32] {
    let message = state_lineage_message_v1(
        operation,
        context_digest,
        current_head,
        current_lineage_digest,
        from_sequence,
        to_sequence,
        request_digest,
        parent_1,
        link,
        amount,
    );
    Sha256::digest(message.as_slice()).into()
}

/// Canonical deterministic sender split seed.
#[allow(clippy::too_many_arguments)]
pub(super) fn offline_cash_send_split_seed_v1(
    context_digest: &[u8; 32],
    wallet_binding: &[u8; 32],
    current_head: &[u8; 32],
    current_opening: &[u8; 32],
    guard_sequence: u64,
    request_digest: &[u8; 32],
    receiver_head: &[u8; 32],
    recipient_key_reference: &[u8; 32],
    amount: u128,
) -> Zeroizing<[u8; 32]> {
    let message = send_split_seed_message_v1(
        context_digest,
        wallet_binding,
        current_head,
        current_opening,
        guard_sequence,
        request_digest,
        receiver_head,
        recipient_key_reference,
        amount,
    );
    Zeroizing::new(Sha256::digest(message.as_slice()).into())
}

/// Canonical deterministic sender remainder and receiver-credit openings.
pub(super) fn offline_cash_send_split_openings_v1(
    split_seed: &[u8; 32],
) -> (Zeroizing<[u8; 32]>, Zeroizing<[u8; 32]>) {
    let sender = send_split_branch_message_v1(split_seed, SEND_SPLIT_SENDER_BRANCH_V1);
    let receiver = send_split_branch_message_v1(split_seed, SEND_SPLIT_RECEIVER_BRANCH_V1);
    (
        Zeroizing::new(Sha256::digest(sender.as_slice()).into()),
        Zeroizing::new(Sha256::digest(receiver.as_slice()).into()),
    )
}

/// Canonical Core/circuit credit commitment.
pub(super) fn offline_cash_credit_head_v1(
    context_digest: &[u8; 32],
    request_digest: &[u8; 32],
    receiver_head: &[u8; 32],
    recipient_key_reference: &[u8; 32],
    amount: u128,
    opening: &[u8; 32],
) -> [u8; 32] {
    let message = credit_head_message_v1(
        context_digest,
        request_digest,
        receiver_head,
        recipient_key_reference,
        amount,
        opening,
    );
    Sha256::digest(message.as_slice()).into()
}

/// Canonical deterministic receiver successor opening.
pub(super) fn offline_cash_receive_opening_v1(
    context_digest: &[u8; 32],
    before_opening: &[u8; 32],
    credit_opening: &[u8; 32],
    request_digest: &[u8; 32],
    send_transition_digest: &[u8; 32],
    amount: u128,
) -> Zeroizing<[u8; 32]> {
    let message = receive_opening_message_v1(
        context_digest,
        before_opening,
        credit_opening,
        request_digest,
        send_transition_digest,
        amount,
    );
    Zeroizing::new(Sha256::digest(message.as_slice()).into())
}

/// Canonical deterministic receiver transition digest.
#[allow(clippy::too_many_arguments)]
pub(super) fn offline_cash_receive_transition_digest_v1(
    context_digest: &[u8; 32],
    balance_parent: &[u8; 32],
    credit_parent: &[u8; 32],
    request_digest: &[u8; 32],
    send_transition_digest: &[u8; 32],
    amount: u128,
    next_amount: u128,
    next_head: &[u8; 32],
) -> [u8; 32] {
    let message = receive_transition_message_v1(
        context_digest,
        balance_parent,
        credit_parent,
        request_digest,
        send_transition_digest,
        amount,
        next_amount,
        next_head,
    );
    Sha256::digest(message.as_slice()).into()
}

/// Canonical deterministic receiver semantic digest.
#[allow(clippy::too_many_arguments)]
pub(super) fn offline_cash_receive_semantic_digest_v1(
    release_id: &[u8; 32],
    context_digest: &[u8; 32],
    request_digest: &[u8; 32],
    balance_parent: &[u8; 32],
    credit_parent: &[u8; 32],
    next_head: &[u8; 32],
    send_transition_digest: &[u8; 32],
    receive_transition_digest: &[u8; 32],
    amount: u128,
    scale: u32,
) -> [u8; 32] {
    let message = receive_semantic_message_v1(
        release_id,
        context_digest,
        request_digest,
        balance_parent,
        credit_parent,
        next_head,
        send_transition_digest,
        receive_transition_digest,
        amount,
        scale,
    );
    Sha256::digest(message.as_slice()).into()
}

/// Move-only private relation witness. It has no wire codec or public export.
#[must_use]
pub(super) struct OfflineCashStatePrivateWitnessV1 {
    pub(super) operation: OfflineCashStateOperationV1,
    pub(super) before_amount: u128,
    pub(super) after_amount: u128,
    pub(super) before_opening: [u8; 32],
    pub(super) after_opening: [u8; 32],
    pub(super) credit_opening: [u8; 32],
    pub(super) wallet_binding: [u8; 32],
    pub(super) guard_device_id: [u8; 32],
    pub(super) hardware_policy_id: [u8; 32],
    pub(super) guard_sequence: u64,
    pub(super) lineage_digest: [u8; 32],
    pub(super) next_lineage_digest: [u8; 32],
    pub(super) send_split_seed: [u8; 32],
    pub(super) recipient_key_reference: [u8; 32],
    pub(super) network_id_frame: [u8; NETWORK_ID_FRAME_BYTES_V1],
    pub(super) asset_id_frame: [u8; ASSET_ID_FRAME_BYTES_V1],
    pub(super) send_transition_message: [u8; SEND_TRANSITION_MESSAGE_BYTES_V1],
    pub(super) send_semantic_message: [u8; SEND_SEMANTIC_MESSAGE_BYTES_V1],
}

impl fmt::Debug for OfflineCashStatePrivateWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashStatePrivateWitnessV1")
            .field("operation", &self.operation)
            .field("amounts", &"[REDACTED]")
            .field("openings", &"[REDACTED]")
            .field("bindings", &"[REDACTED]")
            .finish()
    }
}

impl Drop for OfflineCashStatePrivateWitnessV1 {
    fn drop(&mut self) {
        self.before_amount.zeroize();
        self.after_amount.zeroize();
        self.before_opening.zeroize();
        self.after_opening.zeroize();
        self.credit_opening.zeroize();
        self.wallet_binding.zeroize();
        self.guard_device_id.zeroize();
        self.hardware_policy_id.zeroize();
        self.guard_sequence.zeroize();
        self.lineage_digest.zeroize();
        self.next_lineage_digest.zeroize();
        self.send_split_seed.zeroize();
        self.recipient_key_reference.zeroize();
        self.network_id_frame.zeroize();
        self.asset_id_frame.zeroize();
        self.send_transition_message.zeroize();
        self.send_semantic_message.zeroize();
    }
}

impl OfflineCashStatePrivateWitnessV1 {
    #[allow(clippy::too_many_arguments)]
    fn new(
        operation: OfflineCashStateOperationV1,
        before_amount: u128,
        after_amount: u128,
        before_opening: [u8; 32],
        after_opening: [u8; 32],
        credit_opening: [u8; 32],
        wallet_binding: [u8; 32],
        guard_device_id: [u8; 32],
        hardware_policy_id: [u8; 32],
        guard_sequence: u64,
        lineage_digest: [u8; 32],
        next_lineage_digest: [u8; 32],
        send_split_seed: [u8; 32],
        recipient_key_reference: [u8; 32],
        network_id_frame: [u8; NETWORK_ID_FRAME_BYTES_V1],
        asset_id_frame: [u8; ASSET_ID_FRAME_BYTES_V1],
        send_transition_message: [u8; SEND_TRANSITION_MESSAGE_BYTES_V1],
        send_semantic_message: [u8; SEND_SEMANTIC_MESSAGE_BYTES_V1],
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        let witness = Self {
            operation,
            before_amount,
            after_amount,
            before_opening,
            after_opening,
            credit_opening,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            guard_sequence,
            lineage_digest,
            next_lineage_digest,
            send_split_seed,
            recipient_key_reference,
            network_id_frame,
            asset_id_frame,
            send_transition_message,
            send_semantic_message,
        };
        if witness.before_opening == [0; 32]
            || witness.after_opening == [0; 32]
            || witness.credit_opening == [0; 32]
            || witness.wallet_binding == [0; 32]
            || witness.guard_device_id == [0; 32]
            || witness.hardware_policy_id == [0; 32]
            || witness.lineage_digest == [0; 32]
            || witness.next_lineage_digest == [0; 32]
            || (operation == OfflineCashStateOperationV1::SendSplit
                && witness.send_split_seed == [0; 32])
            || witness.recipient_key_reference == [0; 32]
            || (operation == OfflineCashStateOperationV1::SendSplit
                && (witness.network_id_frame == [0; NETWORK_ID_FRAME_BYTES_V1]
                    || witness.asset_id_frame == [0; ASSET_ID_FRAME_BYTES_V1]
                    || witness.send_transition_message == [0; SEND_TRANSITION_MESSAGE_BYTES_V1]
                    || witness.send_semantic_message == [0; SEND_SEMANTIC_MESSAGE_BYTES_V1]))
            || (operation == OfflineCashStateOperationV1::ReceiveFold
                && (witness.network_id_frame != [0; NETWORK_ID_FRAME_BYTES_V1]
                    || witness.asset_id_frame != [0; ASSET_ID_FRAME_BYTES_V1]
                    || witness.send_transition_message != [0; SEND_TRANSITION_MESSAGE_BYTES_V1]
                    || witness.send_semantic_message != [0; SEND_SEMANTIC_MESSAGE_BYTES_V1]))
        {
            return Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(witness)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn send_split(
        before_amount: u128,
        after_amount: u128,
        before_opening: [u8; 32],
        after_opening: [u8; 32],
        credit_opening: [u8; 32],
        wallet_binding: [u8; 32],
        guard_device_id: [u8; 32],
        hardware_policy_id: [u8; 32],
        guard_sequence: u64,
        lineage_digest: [u8; 32],
        next_lineage_digest: [u8; 32],
        send_split_seed: [u8; 32],
        recipient_key_reference: [u8; 32],
        statement: &OfflineCashTransferStatementV1,
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        let network_id_frame = norito::encode_canonical(&statement.network_id)
            .ok()
            .and_then(zeroizing_exact_array::<NETWORK_ID_FRAME_BYTES_V1>)
            .ok_or(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)?;
        let asset_id_frame = norito::encode_canonical(&statement.asset)
            .ok()
            .and_then(zeroizing_exact_array::<ASSET_ID_FRAME_BYTES_V1>)
            .ok_or(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)?;
        let send_transition_message = statement
            .canonical_transition_digest_message()
            .ok()
            .and_then(zeroizing_exact_array::<SEND_TRANSITION_MESSAGE_BYTES_V1>)
            .ok_or(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)?;
        let send_semantic_message = statement
            .canonical_semantic_digest_message()
            .ok()
            .and_then(zeroizing_exact_array::<SEND_SEMANTIC_MESSAGE_BYTES_V1>)
            .ok_or(OfflineCashStateAbiErrorV1::InvalidPrivateWitness)?;
        Self::new(
            OfflineCashStateOperationV1::SendSplit,
            before_amount,
            after_amount,
            before_opening,
            after_opening,
            credit_opening,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            guard_sequence,
            lineage_digest,
            next_lineage_digest,
            send_split_seed,
            recipient_key_reference,
            network_id_frame,
            asset_id_frame,
            send_transition_message,
            send_semantic_message,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn receive_fold(
        before_amount: u128,
        after_amount: u128,
        before_opening: [u8; 32],
        after_opening: [u8; 32],
        credit_opening: [u8; 32],
        wallet_binding: [u8; 32],
        guard_device_id: [u8; 32],
        hardware_policy_id: [u8; 32],
        guard_sequence: u64,
        lineage_digest: [u8; 32],
        next_lineage_digest: [u8; 32],
        recipient_key_reference: [u8; 32],
    ) -> Result<Self, OfflineCashStateAbiErrorV1> {
        Self::new(
            OfflineCashStateOperationV1::ReceiveFold,
            before_amount,
            after_amount,
            before_opening,
            after_opening,
            credit_opening,
            wallet_binding,
            guard_device_id,
            hardware_policy_id,
            guard_sequence,
            lineage_digest,
            next_lineage_digest,
            [0; 32],
            recipient_key_reference,
            [0; NETWORK_ID_FRAME_BYTES_V1],
            [0; ASSET_ID_FRAME_BYTES_V1],
            [0; SEND_TRANSITION_MESSAGE_BYTES_V1],
            [0; SEND_SEMANTIC_MESSAGE_BYTES_V1],
        )
    }

    pub(super) fn validate_against(
        &self,
        instances: &OfflineCashStatePublicInstancesV1,
    ) -> Result<(), OfflineCashStateAbiErrorV1> {
        let public = instances.relation_public()?;
        if public.operation != self.operation || !self.relation_matches(&public) {
            return Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(())
    }

    pub(super) fn validate_against_leaf(
        &self,
        instances: &OfflineCashStateLeafPublicInstancesV1,
    ) -> Result<(), OfflineCashStateAbiErrorV1> {
        let public = instances.relation_public()?;
        if public.operation != self.operation || !self.relation_matches(&public) {
            return Err(OfflineCashStateAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(())
    }

    fn relation_matches(&self, public: &OfflineCashStateRelationPublicV1) -> bool {
        if public.transfer == 0 {
            return false;
        }
        let conserved = match self.operation {
            OfflineCashStateOperationV1::SendSplit => self
                .after_amount
                .checked_add(public.transfer)
                .is_some_and(|amount| amount == self.before_amount),
            OfflineCashStateOperationV1::ReceiveFold => self
                .before_amount
                .checked_add(public.transfer)
                .is_some_and(|amount| amount == self.after_amount),
        };
        if !conserved {
            return false;
        }
        let Some(next_sequence) = self.guard_sequence.checked_add(1) else {
            return false;
        };
        let before = offline_cash_balance_head_v1(
            &public.context_digest,
            &self.wallet_binding,
            &self.guard_device_id,
            &self.hardware_policy_id,
            self.guard_sequence,
            &self.lineage_digest,
            self.before_amount,
            &self.before_opening,
        );
        let expected_next_lineage = offline_cash_state_lineage_digest_v1(
            self.operation,
            &public.context_digest,
            &public.parent_0,
            &self.lineage_digest,
            self.guard_sequence,
            next_sequence,
            &public.request_digest,
            &public.parent_1,
            &public.link,
            public.transfer,
        );
        let after = offline_cash_balance_head_v1(
            &public.context_digest,
            &self.wallet_binding,
            &self.guard_device_id,
            &self.hardware_policy_id,
            next_sequence,
            &self.next_lineage_digest,
            self.after_amount,
            &self.after_opening,
        );
        let receiver_head = match self.operation {
            OfflineCashStateOperationV1::SendSplit => public.parent_1,
            OfflineCashStateOperationV1::ReceiveFold => public.parent_0,
        };
        let credit = offline_cash_credit_head_v1(
            &public.context_digest,
            &public.request_digest,
            &receiver_head,
            &self.recipient_key_reference,
            public.transfer,
            &self.credit_opening,
        );
        let expected_credit = match self.operation {
            OfflineCashStateOperationV1::SendSplit => public.link,
            OfflineCashStateOperationV1::ReceiveFold => public.parent_1,
        };
        if before != public.parent_0
            || self.next_lineage_digest != expected_next_lineage
            || after != public.result
            || credit != expected_credit
        {
            return false;
        }
        if self.operation == OfflineCashStateOperationV1::SendSplit {
            let Ok(network_id) = norito::decode_canonical::<NetworkId>(&self.network_id_frame)
            else {
                return false;
            };
            let Ok(asset) = norito::decode_canonical::<AssetDefinitionId>(&self.asset_id_frame)
            else {
                return false;
            };
            let Some(statement_frame) = self
                .send_semantic_message
                .get(SEND_SEMANTIC_DIGEST_PREFIX_BYTES_V1..)
            else {
                return false;
            };
            let Ok(statement) =
                norito::decode_canonical::<OfflineCashTransferStatementV1>(statement_frame)
            else {
                return false;
            };
            if statement.version != OFFLINE_CASH_WIRE_VERSION_V1
                || statement.release_id != public.release_id
                || statement.network_id != network_id
                || statement.asset != asset
                || statement.scale != public.scale
                || statement.amount != public.transfer
                || statement.request_digest != public.request_digest
                || statement.sender_before != public.parent_0
                || statement.sender_after != public.result
                || statement.receiver_before != public.parent_1
                || statement.credit_commitment != public.link
                || statement.transition_digest != public.transition_digest
                || statement
                    .canonical_transition_digest_message()
                    .ok()
                    .is_none_or(|message| {
                        message.as_slice() != self.send_transition_message.as_slice()
                    })
                || statement
                    .canonical_semantic_digest_message()
                    .ok()
                    .is_none_or(|message| {
                        message.as_slice() != self.send_semantic_message.as_slice()
                    })
            {
                return false;
            }
            let expected_seed = offline_cash_send_split_seed_v1(
                &public.context_digest,
                &self.wallet_binding,
                &public.parent_0,
                &self.before_opening,
                self.guard_sequence,
                &public.request_digest,
                &public.parent_1,
                &self.recipient_key_reference,
                public.transfer,
            );
            let (expected_after_opening, expected_credit_opening) =
                offline_cash_send_split_openings_v1(&expected_seed);
            let context_message = state_context_message_v1(
                &public.release_id,
                &self.network_id_frame,
                &self.asset_id_frame,
                public.scale,
            );
            return self.send_split_seed == *expected_seed
                && self.after_opening == *expected_after_opening
                && self.credit_opening == *expected_credit_opening
                && <[u8; 32]>::from(Sha256::digest(context_message.as_slice()))
                    == public.context_digest
                && <[u8; 32]>::from(Sha256::digest(self.send_transition_message.as_slice()))
                    == public.transition_digest
                && <[u8; 32]>::from(Sha256::digest(self.send_semantic_message.as_slice()))
                    == public.semantic_digest;
        }
        let expected_after_opening = offline_cash_receive_opening_v1(
            &public.context_digest,
            &self.before_opening,
            &self.credit_opening,
            &public.request_digest,
            &public.link,
            public.transfer,
        );
        if self.after_opening != *expected_after_opening {
            return false;
        }
        let expected_transition = offline_cash_receive_transition_digest_v1(
            &public.context_digest,
            &public.parent_0,
            &public.parent_1,
            &public.request_digest,
            &public.link,
            public.transfer,
            self.after_amount,
            &public.result,
        );
        if public.transition_digest != expected_transition {
            return false;
        }
        public.semantic_digest
            == offline_cash_receive_semantic_digest_v1(
                &public.release_id,
                &public.context_digest,
                &public.request_digest,
                &public.parent_0,
                &public.parent_1,
                &public.result,
                &public.link,
                &public.transition_digest,
                public.transfer,
                public.scale,
            )
    }

    #[cfg(test)]
    pub(super) fn corrupt_before_opening_for_test(&mut self) {
        self.before_opening[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_after_amount_for_test(&mut self) {
        self.after_amount ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_credit_opening_for_test(&mut self) {
        self.credit_opening[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_after_opening_for_test(&mut self) {
        self.after_opening[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_guard_sequence_for_test(&mut self) {
        self.guard_sequence ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_lineage_for_test(&mut self) {
        self.lineage_digest[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_next_lineage_for_test(&mut self) {
        self.next_lineage_digest[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_send_split_seed_for_test(&mut self) {
        self.send_split_seed[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_network_frame_for_test(&mut self) {
        self.network_id_frame[0] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_asset_frame_for_test(&mut self) {
        self.asset_id_frame[6] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_send_transition_message_for_test(&mut self) {
        self.send_transition_message[SEND_TRANSITION_REQUEST_OFFSET_V1] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn corrupt_send_semantic_length_for_test(&mut self) {
        self.send_semantic_message[43] ^= 1;
    }

    #[cfg(test)]
    pub(super) fn reorder_send_semantic_fields_for_test(&mut self) {
        for index in 0..DIGEST_BYTES {
            self.send_semantic_message.swap(
                SEND_SEMANTIC_BEFORE_OFFSET_V1 + index,
                SEND_SEMANTIC_AFTER_OFFSET_V1 + index,
            );
        }
    }

    #[cfg(test)]
    pub(super) fn replace_send_transition_message_for_test(
        &mut self,
        message: &[u8],
    ) -> Result<(), OfflineCashStateAbiErrorV1> {
        let replacement: [u8; SEND_TRANSITION_MESSAGE_BYTES_V1] = message
            .try_into()
            .map_err(|_| OfflineCashStateAbiErrorV1::InvalidPrivateWitness)?;
        self.send_transition_message.zeroize();
        self.send_transition_message = replacement;
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn zero_rejected_field_for_test(&mut self, index: usize) {
        match index {
            0 => self.before_opening.zeroize(),
            1 => self.after_opening.zeroize(),
            2 => self.credit_opening.zeroize(),
            3 => self.wallet_binding.zeroize(),
            4 => self.guard_device_id.zeroize(),
            5 => self.hardware_policy_id.zeroize(),
            6 => self.recipient_key_reference.zeroize(),
            7 => self.lineage_digest.zeroize(),
            8 => self.next_lineage_digest.zeroize(),
            9 => self.send_split_seed.zeroize(),
            _ => panic!("unknown rejected private STATE field"),
        }
    }
}
